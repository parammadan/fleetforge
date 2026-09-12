//! The `fleetforge` binary.
//!
//! Read-only. This process constructs no mutating Kubernetes client, because no
//! execution adapter exists (ADR-0012), and binds to loopback by default
//! because it has no authentication of its own yet.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use ff_api::routes;
use ff_api::state::{AppState, DataSource};
use ff_collect::{CollectorConfig, FixtureSource};
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Args {
    fixtures: Option<PathBuf>,
    kubeconfig: Option<PathBuf>,
    context: Option<String>,
    bind: SocketAddr,
    once: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        fixtures: None,
        kubeconfig: None,
        context: None,
        // Loopback, deliberately. FleetForge has no authentication yet, so it
        // must not be reachable from the network (THREAT_MODEL.md, known gaps).
        bind: "127.0.0.1:8080".parse().map_err(|_| "bad default bind")?,
        once: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--fixtures" => args.fixtures = it.next().map(PathBuf::from),
            "--kubeconfig" => args.kubeconfig = it.next().map(PathBuf::from),
            "--context" => args.context = it.next(),
            "--bind" => {
                let raw = it.next().ok_or("--bind needs an address")?;
                args.bind = raw.parse().map_err(|_| format!("bad address: {raw}"))?;
            }
            "--once" => args.once = true,
            "--help" | "-h" => {
                println!(
                    "fleetforge {VERSION}\n\n\
                     Read-only Kubernetes node-maintenance intelligence.\n\n\
                     USAGE:\n  \
                       fleetforge [--kubeconfig PATH] [--context NAME] [--bind ADDR]\n  \
                       fleetforge --fixtures DIR\n  \
                       fleetforge --once            print one snapshot summary and exit\n"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(args)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                // kube_runtime logs the raw API error, which includes the requesting
                // identity and full resource path. ff-collect emits its own
                // redacted line for the same event (THREAT_MODEL.md R1), so the
                // unredacted one is silenced rather than duplicated.
                .unwrap_or_else(|_| "info,kube=warn,kube_runtime::watcher=error".into()),
        )
        .init();

    let args = parse_args()?;

    let state = if let Some(dir) = &args.fixtures {
        let snapshot = FixtureSource::new(dir).load()?;
        tracing::warn!(
            mode = "FIXTURE",
            "serving recorded data from disk; this is NOT a live cluster"
        );
        Arc::new(AppState {
            source: DataSource::Fixture(Arc::new(snapshot)),
            started_at: Utc::now(),
            version: VERSION,
        })
    } else {
        let config = CollectorConfig {
            kubeconfig: args.kubeconfig.clone(),
            context: args.context.clone(),
            ..CollectorConfig::default()
        };

        let collector = ff_collect::start(config).await?;
        tracing::info!(
            cluster_id = collector.cluster_id(),
            server_version = collector.server_version(),
            client_target = "v1.36",
            "connected read-only"
        );
        Arc::new(AppState {
            source: DataSource::Live(Arc::new(collector)),
            started_at: Utc::now(),
            version: VERSION,
        })
    };

    if args.once {
        return print_once(&state).await;
    }

    let app = routes::router(Arc::clone(&state))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        // The frontend dev server runs on a different port. Permissive CORS is
        // acceptable only because this binds to loopback and serves read-only
        // data; it must be revisited before any mutating endpoint exists.
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    tracing::info!(address = %args.bind, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shut down cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown requested");
}

/// Print one snapshot summary. Used by the live smoke test.
async fn print_once(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    // Wait for the first sync rather than reporting an empty cluster.
    for _ in 0..60 {
        if state.snapshot().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    let Some(snapshot) = state.snapshot() else {
        return Err("first sync did not complete within 15s".into());
    };

    println!("mode            {}", snapshot.mode().label());
    println!("cluster_id      {}", snapshot.cluster_id());
    println!("snapshot_id     {}", snapshot.snapshot_id().short());
    println!("taken_at        {}", snapshot.taken_at());
    println!("authoritative   {}", state.authoritative().await);
    println!();
    println!("{:<22} {:>6}  COLLECTION STATUS", "KIND", "COUNT");
    for c in state.coverage().await {
        println!(
            "{:<22} {:>6}  {}",
            c.kind,
            c.observed_count,
            c.status.label()
        );
    }
    println!();
    for node in snapshot.nodes() {
        println!(
            "node  {:<30} ready={:<5} cpu={:<8} mem={:<8} rv={}",
            node.name,
            node.is_ready(),
            node.allocatable_cpu,
            node.allocatable_memory,
            node.provenance.resource_version().unwrap_or("-")
        );
    }
    for pdb in snapshot.pdbs() {
        println!(
            "pdb   {}/{:<24} disruptionsAllowed={} blocks={}",
            pdb.namespace,
            pdb.name,
            pdb.disruptions_allowed,
            pdb.blocks_disruption()
        );
    }
    Ok(())
}

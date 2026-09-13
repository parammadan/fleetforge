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
    record: Option<PathBuf>,
    run_description: String,
    fixtures: Option<PathBuf>,
    replay: Option<PathBuf>,
    ui: Option<PathBuf>,
    kubeconfig: Option<PathBuf>,
    context: Option<String>,
    bind: SocketAddr,
    once: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        record: None,
        run_description: "unnamed run".to_owned(),
        fixtures: None,
        replay: None,
        ui: None,
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
            "--record" => args.record = it.next().map(PathBuf::from),
            "--run-description" => {
                args.run_description = it.next().unwrap_or_else(|| "unnamed run".to_owned());
            }
            "--fixtures" => args.fixtures = it.next().map(PathBuf::from),
            "--replay" => args.replay = it.next().map(PathBuf::from),
            "--ui" => args.ui = it.next().map(PathBuf::from),
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
                       fleetforge --replay DIR      replay a captured evidence bundle\n  \
                       fleetforge --ui DIR          also serve a built interface at /\n  \
                       fleetforge --once            print one snapshot summary and exit\n\n\
                     RECORDING:\n  \
                       --record PATH                append events to a JSONL log\n  \
                       --run-description TEXT       what this run is for\n"
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

    let state = if let Some(dir) = &args.replay {
        let bundle = ff_replay::ReplayBundle::load(dir)?;
        tracing::warn!(
            mode = "REPLAY",
            cluster_id = %bundle.context.cluster_id,
            events = bundle.timeline.len(),
            from = %bundle.context.captured_from,
            to = %bundle.context.captured_to,
            "replaying a captured incident; this is NOT a live cluster"
        );
        Arc::new(AppState {
            source: DataSource::Replay(Arc::new(bundle)),
            started_at: Utc::now(),
            version: VERSION,
            // Replay is read-only over an existing log. Recording a replay of a
            // recording would produce an event log that looks live but is not.
            log: None,
        })
    } else if let Some(dir) = &args.fixtures {
        let snapshot = FixtureSource::new(dir).load()?;
        tracing::warn!(
            mode = "FIXTURE",
            "serving recorded data from disk; this is NOT a live cluster"
        );
        Arc::new(AppState {
            source: DataSource::Fixture(Arc::new(snapshot)),
            started_at: Utc::now(),
            version: VERSION,
            log: None,
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
        let cluster_id = collector.cluster_id().to_owned();
        let log = match &args.record {
            Some(path) => {
                let started = Utc::now();
                let log = Arc::new(ff_record::EventLog::open(
                    path,
                    ff_record::RunId::from_time(started),
                    ff_core::Mode::Live,
                )?);
                log.record(ff_record::RecordedEvent::RunStarted {
                    description: args.run_description.clone(),
                    cluster_id: cluster_id.clone(),
                });
                tracing::info!(
                    path = %path.display(),
                    run_id = %log.run_id(),
                    "recording to an append-only JSONL log"
                );
                Some(log)
            }
            None => None,
        };

        Arc::new(AppState {
            source: DataSource::Live(Arc::new(collector)),
            started_at: Utc::now(),
            version: VERSION,
            log,
        })
    };

    // Start recording before serving, so the log covers the whole session.
    if let Some(log) = state.log.clone() {
        ff_api::recorder::spawn(Arc::clone(&state), log);
    }

    if args.once {
        return print_once(&state).await;
    }

    // Validated before binding: a process that starts happily and then serves
    // 404s to the room is worse than one that refuses to start and says why.
    let ui = match &args.ui {
        Some(dir) => Some(ff_api::ui::validate(dir)?),
        None => None,
    };

    let mut app = routes::router(Arc::clone(&state));
    app = match &ui {
        Some(dir) => {
            tracing::info!(path = %dir.display(), "serving the built interface at /");
            ff_api::ui::mount(app, dir)
        }
        None => app.fallback(ff_api::ui::no_ui),
    };

    let app = app
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        // The frontend dev server runs on a different port. Permissive CORS is
        // acceptable only because this binds to loopback and serves read-only
        // data; it must be revisited before any mutating endpoint exists.
        .layer(CorsLayer::permissive());

    let listener = match tokio::net::TcpListener::bind(args.bind).await {
        Ok(l) => l,
        // The commonest failure by far, and the default message ("Address
        // already in use") does not say which address or what to do about it.
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            return Err(format!(
                "port {} is already in use.\n\
                 Something else is listening on {}. Stop it, or choose another port with \
                 --bind 127.0.0.1:<port>.\n\
                 To find it:  lsof -nP -iTCP:{} -sTCP:LISTEN",
                args.bind.port(),
                args.bind,
                args.bind.port(),
            )
            .into());
        }
        Err(e) => return Err(e.into()),
    };

    if ui.is_some() {
        // The one line a presenter needs. Deliberately not behind the tracing
        // filter, which an operator may have turned down.
        println!("\n  FleetForge replay  →  http://{}\n", args.bind);
    }
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

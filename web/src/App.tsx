import { useEffect, useState } from "react";

import { useCluster } from "./useCluster";
import {
  EnvironmentPanel,
  EventsPanel,
  ModeBadge,
  NodesPanel,
  PdbPanel,
  StreamIndicator,
  TopologyPanel,
  WorkloadsPanel,
} from "./components";
import { PreflightWorkspace } from "./PreflightWorkspace";
import { BrupopPanel, ReportPanel } from "./Report";
import { ReplayControlRoom } from "./ReplayControlRoom";
import type { CollectionStatus, Environment, KindCoverage } from "./types";

function statusFor(coverage: KindCoverage[], kind: string): CollectionStatus | undefined {
  return coverage.find((c) => c.kind === kind)?.status;
}

/**
 * Which interface to show.
 *
 * Decided by the backend, from one cheap request, before any other data is
 * fetched. Replay and live are different products with different obligations —
 * one narrates a past incident, the other reports a present cluster — and
 * choosing between them per-panel is how a live badge ends up over replay data.
 *
 * Hooks cannot be called conditionally, so the branch has to happen here, above
 * both `useCluster` and `useReplayBundle`.
 */
type Route =
  | { kind: "deciding" }
  | { kind: "replay" }
  | { kind: "live" }
  | { kind: "unreachable"; message: string };

function useRoute(baseUrl = ""): Route {
  const [route, setRoute] = useState<Route>({ kind: "deciding" });

  useEffect(() => {
    let cancelled = false;
    fetch(`${baseUrl}/api/v1/environment`)
      .then(async (r) => {
        if (!r.ok) throw new Error(`environment: HTTP ${r.status}`);
        return (await r.json()) as Environment;
      })
      .then((env) => {
        if (cancelled) return;
        setRoute(env.mode === "replay" ? { kind: "replay" } : { kind: "live" });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setRoute({
          kind: "unreachable",
          message: e instanceof Error ? e.message : String(e),
        });
      });
    return () => {
      cancelled = true;
    };
  }, [baseUrl]);

  return route;
}

export default function App() {
  const route = useRoute();

  return (
    <>
      {/* Outside the branch on purpose. A keyboard user who lands while the
          mode is still being decided needs somewhere to tab to, and the skip
          link is the first stop on every screen this app has.

          `tabIndex={0}` is not redundant: Safari and other WebKit browsers do
          not put plain links in the tab order unless the user has turned on
          Full Keyboard Access, so without it the skip link is unreachable by
          keyboard on macOS Safari — which is exactly the browser a reviewer
          opening this from a Mac will use. Found by the WebKit run. */}
      <a className="skip-link" href="#content" tabIndex={0}>
        Skip to content
      </a>
      <Routed route={route} />
    </>
  );
}

function Routed({ route }: { route: Route }) {
  if (route.kind === "deciding") {
    return (
      <main id="content">
        <section className="panel">
          <div className="state-block">
            <div className="spinner" aria-hidden="true" />
            <h3>Connecting to FleetForge</h3>
            <p>Asking the backend what kind of data it is serving.</p>
          </div>
        </section>
      </main>
    );
  }

  if (route.kind === "unreachable") {
    return (
      <main id="content">
        <section className="panel">
          <div className="state-block">
            <h3 className="state-forbidden">FleetForge is not reachable</h3>
            <p className="mono">{route.message}</p>
            <p>
              Nothing is shown, because there is no honest thing to show. Start the backend
              and reload.
            </p>
          </div>
        </section>
      </main>
    );
  }

  return route.kind === "replay" ? <ReplayControlRoom /> : <LiveDashboard />;
}

function LiveDashboard() {
  const { snapshot, environment, stream, authoritative, error, updateCount } = useCluster();
  const coverage = snapshot?.coverage ?? environment?.coverage ?? [];
  const mode = snapshot?.mode ?? environment?.mode ?? "live";
  const modeLabel = environment?.mode_label ?? mode.toUpperCase();

  return (
    <>
      {/* Persistent chrome. The mode badge has no dismiss control (ADR-0003). */}
      <header className="chrome">
        <span className="brand">
          FleetForge <span>· node-maintenance intelligence</span>
        </span>
        <ModeBadge mode={mode} label={modeLabel} />
        <StreamIndicator stream={stream} mode={mode} />
        <span className="chrome-spacer" />
        <span className="meta">
          {snapshot ? `snapshot ${snapshot.snapshot_id.slice(0, 12)}` : "no snapshot yet"}
          {updateCount > 0 && ` · ${updateCount} update${updateCount === 1 ? "" : "s"}`}
        </span>
      </header>

      {mode === "fixture" && (
        <div className="banner banner-fixture">
          <strong>FIXTURE</strong>
          <span>
            This is recorded data loaded from disk. It was captured from a real cluster, which
            does not make it live — nothing here reflects any cluster's current state.
          </span>
        </div>
      )}

      {!authoritative && (
        <div className="banner banner-danger">
          <strong>Incomplete view</strong>
          <span>
            At least one resource kind could not be collected. Absence below does not mean
            absence in the cluster — check the environment panel for which kind and why.
          </span>
        </div>
      )}

      {stream.kind === "disconnected" && (
        <div className="banner banner-danger">
          <strong>Disconnected</strong>
          <span>
            The event stream dropped and is retrying. Everything below is frozen at the last
            update received and is no longer current.
          </span>
        </div>
      )}

      {stream.kind === "stale" && (
        <div className="banner banner-warn">
          <strong>Stale</strong>
          <span>
            No heartbeat has arrived recently. The stream may be dead — this is not the same as
            a quiet cluster.
          </span>
        </div>
      )}

      {error && (
        <div className="banner banner-warn">
          <strong>Error</strong>
          <span>{error}</span>
        </div>
      )}

      <main id="content">
        {!snapshot ? (
          <section className="panel">
            <div className="state-block">
              <div className="spinner" aria-hidden="true" />
              <h3>Waiting for the first cluster sync</h3>
              <p>
                FleetForge is establishing its watches. Nothing is displayed until it has a
                complete view — an empty screen here would be indistinguishable from an empty
                cluster.
              </p>
            </div>
          </section>
        ) : (
          <div className="grid">
            <NodesPanel nodes={snapshot.nodes} status={statusFor(coverage, "Node")} />
            <PdbPanel
              pdbs={snapshot.pdbs}
              status={statusFor(coverage, "PodDisruptionBudget")}
            />
            <WorkloadsPanel
              workloads={snapshot.workloads}
              status={statusFor(coverage, "Deployment")}
            />
            <BrupopPanel
              shadows={snapshot.brupop ?? []}
              status={statusFor(coverage, "BottlerocketShadow")}
            />
            <EnvironmentPanel
              coverage={coverage}
              clusterId={snapshot.cluster_id}
              serverVersion={environment?.server_version ?? null}
              clientTarget={environment?.client_target_version ?? "—"}
              snapshotId={snapshot.snapshot_id}
              version={environment?.version ?? "?"}
            />
            <div style={{ gridColumn: "1 / -1" }}>
              <PreflightWorkspace nodes={snapshot.nodes} />
            </div>
            <div style={{ gridColumn: "1 / -1" }}>
              <TopologyPanel
                nodes={snapshot.nodes}
                pods={snapshot.pods}
                status={statusFor(coverage, "Pod")}
              />
            </div>
            <div style={{ gridColumn: "1 / -1" }}>
              <ReportPanel />
            </div>
            <div style={{ gridColumn: "1 / -1" }}>
              <EventsPanel events={snapshot.events} status={statusFor(coverage, "Event")} />
            </div>
          </div>
        )}
      </main>
    </>
  );
}

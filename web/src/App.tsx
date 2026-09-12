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
import type { CollectionStatus, KindCoverage } from "./types";

function statusFor(coverage: KindCoverage[], kind: string): CollectionStatus | undefined {
  return coverage.find((c) => c.kind === kind)?.status;
}

export default function App() {
  const { snapshot, environment, stream, authoritative, error, updateCount } = useCluster();
  const coverage = snapshot?.coverage ?? environment?.coverage ?? [];
  const mode = snapshot?.mode ?? environment?.mode ?? "live";
  const modeLabel = environment?.mode_label ?? mode.toUpperCase();

  return (
    <>
      <a className="skip-link" href="#content">
        Skip to content
      </a>

      {/* Persistent chrome. The mode badge has no dismiss control (ADR-0003). */}
      <header className="chrome">
        <span className="brand">
          FleetForge <span>· node-maintenance intelligence</span>
        </span>
        <ModeBadge mode={mode} label={modeLabel} />
        <StreamIndicator stream={stream} />
        <span className="chrome-spacer" />
        <span className="meta">
          {snapshot ? `snapshot ${snapshot.snapshot_id.slice(0, 12)}` : "no snapshot yet"}
          {updateCount > 0 && ` · ${updateCount} updates`}
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
              <EventsPanel events={snapshot.events} status={statusFor(coverage, "Event")} />
            </div>
          </div>
        )}
      </main>
    </>
  );
}

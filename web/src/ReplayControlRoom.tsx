// The replay control room.
//
// One screen, answering three audiences' questions in the order they ask them:
// an executive wants to know what happened and what it cost, an operator wants
// to know what the system saw and when, an engineer wants to check the
// arithmetic. Top to bottom is that order.
//
// The banner never leaves. It is not dismissible and it is not a toast: a
// screen full of node names, pod counts and live-looking state is exactly the
// thing someone screenshots, and the screenshot has to carry the label too.

import { ModeBadge } from "./components";
import {
  EvidenceExplorer,
  ExecutiveSummary,
  FleetTopology,
  Limitations,
  PredictedVsActual,
  PreflightFinding,
} from "./ReplayPanels";
import { PositionStrip, ReplayTimeline } from "./ReplayTimeline";
import type { BundleState } from "./useReplay";
import { usePlayback, useReplayBundle } from "./useReplay";

export function ReplayControlRoom({ baseUrl = "" }: { baseUrl?: string }) {
  const state = useReplayBundle(baseUrl);

  if (state.kind === "loading") return <Loading />;
  if (state.kind === "error") return <LoadFailure message={state.message} />;
  return <Loaded state={state} baseUrl={baseUrl} />;
}

function Loaded({
  state,
  baseUrl,
}: {
  state: Extract<BundleState, { kind: "ready" }>;
  baseUrl: string;
}) {
  const { bundle } = state;
  const playback = usePlayback(bundle.timeline, baseUrl);
  const { context } = bundle;

  return (
    <>
      <header className="chrome">
        <span className="brand">
          FleetForge <span>· incident replay</span>
        </span>
        <ModeBadge mode={bundle.mode} label={bundle.modeLabel} />
        <span className="chrome-spacer" />
        <span className="meta">
          {context.context.cluster_kind} · {context.context.kubernetes_version ?? "version unknown"}
        </span>
      </header>

      {/* Persistent, undismissable, and first in the DOM after the header so a
          screen reader reaches it before any cluster data. */}
      <div className="banner banner-replay" role="note">
        <strong>REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION</strong>
        <span>
          Recorded {context.context.captured_from.slice(0, 10)} from a cluster that has since
          been destroyed. Nothing on this screen is live, nothing is simulated, and no value
          has been edited to improve the story. {context.events.toLocaleString()} events,{" "}
          {bundle.artifacts.length} artifacts.
        </span>
      </div>

      <PositionStrip state={playback.state} />

      {playback.error && (
        <div className="banner banner-danger">
          <strong>State unavailable</strong>
          <span>
            {playback.error} — the fleet below is the last position that loaded, not the
            position the timeline is on.
          </span>
        </div>
      )}

      <main id="content">
        <div className="grid">
          <div className="span-full">
            <ExecutiveSummary
              capturedFrom={context.context.captured_from}
              capturedTo={context.context.captured_to}
              brupopFirstSeenAt={context.context.brupop_first_seen_at}
              claims={bundle.claims}
              traffic={bundle.traffic}
              nodeCount={context.context.nodes.length}
            />
          </div>

          <div className="span-full">
            <ReplayTimeline
              timeline={bundle.timeline}
              chapters={bundle.chapters}
              playback={playback}
              totalEvents={context.events}
            />
          </div>

          <div className="span-full">
            <FleetTopology state={playback.state} />
          </div>

          <div className="span-full">
            <PreflightFinding
              pdb={bundle.pdb}
              preflight={playback.state?.last_preflight ?? null}
            />
          </div>

          <div className="span-full">
            <PredictedVsActual rows={bundle.predictions} />
          </div>

          <div className="span-full">
            <EvidenceExplorer artifacts={bundle.artifacts} />
          </div>

          <div className="span-full">
            <Limitations caveats={context.caveats} claims={bundle.claims} />
          </div>
        </div>
      </main>
    </>
  );
}

function Loading() {
  return (
    <main id="content">
      <section className="panel">
        <div className="state-block">
          <div className="spinner" aria-hidden="true" />
          <h3>Loading the evidence bundle</h3>
          <p>
            Reading the captured incident from the Rust API. Nothing is rendered until it has
            answered — a half-loaded control room is indistinguishable from a quiet cluster.
          </p>
        </div>
      </section>
    </main>
  );
}

function LoadFailure({ message }: { message: string }) {
  return (
    <main id="content">
      <section className="panel">
        <div className="state-block">
          <h3 className="state-forbidden">The replay could not be loaded</h3>
          <p className="mono">{message}</p>
          <p>
            Start the backend with <code>fleetforge --replay evidence/eks-recovery</code>.
            This screen shows nothing rather than showing something plausible.
          </p>
        </div>
      </section>
    </main>
  );
}

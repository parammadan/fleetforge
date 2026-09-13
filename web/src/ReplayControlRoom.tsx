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

import { useCallback, useEffect, useRef, useState } from "react";

import { ModeBadge } from "./components";
import { Term } from "./glossary";
import { InvestigationChainPanel } from "./InvestigationChain";
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

  // The evidence drawer is owned here so that a claim in the summary, a link in
  // the investigation chain, and the evidence table itself all open the same
  // one — and so that opening it scrolls the reader to it rather than changing
  // something off screen.
  const [openArtifact, setOpenArtifact] = useState<string | null>(null);
  const evidenceRef = useRef<HTMLDivElement>(null);
  const pendingScroll = useRef(false);

  const openEvidence = useCallback((name: string | null) => {
    setOpenArtifact(name);
    pendingScroll.current = name !== null;
  }, []);

  useEffect(() => {
    if (!pendingScroll.current || !openArtifact) return;
    pendingScroll.current = false;
    evidenceRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [openArtifact]);

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
          has been edited to improve the story.
        </span>
      </div>

      <ProvenanceStrip
        context={context}
        bottlerocket={context.context.bottlerocket_versions}
        artifacts={bundle.artifacts.length}
        state={playback.state}
        stale={playback.stale}
        step={playback.step}
        steps={bundle.timeline.length}
      />

      <PositionStrip
        stale={playback.stale}
        state={playback.state}
        firstObservationAt={bundle.timeline.find((e) => e.kind === "node_changed")?.at}
      />

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
              pdb={bundle.pdb}
              onOpenEvidence={openEvidence}
            />
          </div>

          <div className="span-full">
            <InvestigationChainPanel chain={bundle.chain} onOpenEvidence={openEvidence} />
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
            <FleetTopology state={playback.state} onOpenEvidence={openEvidence} />
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

          <div className="span-full" ref={evidenceRef}>
            <EvidenceExplorer
              artifacts={bundle.artifacts}
              open={openArtifact}
              onOpen={openEvidence}
            />
          </div>

          <div className="span-full">
            <Limitations
              caveats={context.caveats}
              claims={bundle.claims}
              capturedFrom={context.context.captured_from}
              brupopFirstSeenAt={context.context.brupop_first_seen_at}
            />
          </div>
        </div>
      </main>
    </>
  );
}

/**
 * Provenance, always on screen.
 *
 * Capture window, cluster identity, Kubernetes and Bottlerocket versions, where
 * the playhead is, and the hash of the snapshot the current state derives from.
 * A screenshot of this interface should be enough to tell someone exactly which
 * recording, and which moment in it, they are looking at.
 */
function ProvenanceStrip({
  context,
  bottlerocket,
  artifacts,
  state,
  stale,
  step,
  steps,
}: {
  context: { context: { cluster_id: string; kubernetes_version: string | null; captured_from: string; captured_to: string }; events: number };
  bottlerocket: string[];
  artifacts: number;
  state: { snapshot_id: string | null; at: string } | null;
  stale: boolean;
  step: number;
  steps: number;
}) {
  const c = context.context;
  return (
    <dl className="provenance">
      <div>
        <dt>Capture</dt>
        <dd className="mono">
          {c.captured_from.slice(0, 10)} {c.captured_from.slice(11, 19)}–
          {c.captured_to.slice(11, 19)} UTC
        </dd>
      </div>
      <div>
        <dt>Cluster</dt>
        <dd className="mono" title={c.cluster_id}>
          {c.cluster_id.slice(0, 8)}… · destroyed
        </dd>
      </div>
      <div>
        <dt>Kubernetes</dt>
        <dd className="mono">{c.kubernetes_version ?? "unknown"}</dd>
      </div>
      <div>
        <dt>Bottlerocket</dt>
        <dd className="mono">{bottlerocket.join(" → ") || "unknown"}</dd>
      </div>
      <div>
        <dt>Position</dt>
        <dd className="mono">
          {stale ? "…" : state ? state.at.slice(11, 19) : "—"} · step {step + 1}/{steps} of{" "}
          {context.events.toLocaleString()} events
        </dd>
      </div>
      <div>
        <dt>
          <Term k="snapshot">Snapshot</Term>
        </dt>
        <dd className="mono" title={state?.snapshot_id ?? undefined}>
          {state?.snapshot_id ? `${state.snapshot_id.slice(0, 12)}…` : "none yet"}
        </dd>
      </div>
      <div>
        <dt>Evidence</dt>
        <dd className="mono">{artifacts} artifacts</dd>
      </div>
    </dl>
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

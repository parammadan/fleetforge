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
import { ReplayDetails } from "./ReplayDetails";
import type { TabId } from "./ReplayDetails";
import { ReplayOverview } from "./ReplayOverview";
import type { ReplayState } from "./replayTypes";
import { PositionStrip } from "./ReplayTimeline";
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

  // The outcome ("3/3 updated") is the state at the *end* of the capture, not
  // wherever the scrubber happens to be — otherwise the headline figure would
  // change as a reader played the timeline, which is exactly backwards.
  const [finalState, setFinalState] = useState<ReplayState | null>(null);
  useEffect(() => {
    let cancelled = false;
    fetch(`${baseUrl}/api/v1/replay/state?position=${context.events - 1}`)
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
      .then((envelope: { data: ReplayState }) => {
        if (!cancelled) setFinalState(envelope.data);
      })
      .catch(() => {
        // The overview falls back to the node count from the capture context,
        // which is derived from the same bundle. It never invents a number.
      });
    return () => {
      cancelled = true;
    };
  }, [baseUrl, context.events]);

  const [tab, setTab] = useState<TabId>("timeline");
  const detailsRef = useRef<HTMLDivElement>(null);

  // The CTA and any evidence link both land the reader in the same place, with
  // the right tab already open.
  // `scrollIntoView` does not account for sticky headers, so scrolling to the
  // detail region put the tab row half-behind the chrome. Compute the offset
  // from the sticky elements themselves rather than hard-coding a pixel value
  // that drifts the moment the banner wraps to two lines.
  const scrollToDetails = useCallback(() => {
    const target = detailsRef.current;
    if (!target) return;
    // Every element that will still be pinned after the scroll settles. The
    // position strip is sticky *and* lives inside the detail region, so it
    // pins itself directly under the header and hides whatever the scroll
    // brought to the top — which was the tab row.
    const offset = [".site-header", ".position-strip"]
      .map((sel) => document.querySelector<HTMLElement>(sel))
      .filter((el): el is HTMLElement => el !== null)
      .reduce((total, el) => total + el.getBoundingClientRect().height, 0);
    window.scrollTo({
      top: target.getBoundingClientRect().top + window.scrollY - offset,
      behavior: "smooth",
    });
  }, []);

  const goToDetails = useCallback(
    (target?: TabId) => {
      if (target) setTab(target);
      requestAnimationFrame(scrollToDetails);
    },
    [scrollToDetails],
  );

  const openEvidenceFromAnywhere = useCallback(
    (name: string | null) => {
      openEvidence(name);
      if (name) {
        setTab("evidence");
        requestAnimationFrame(scrollToDetails);
      }
    },
    [openEvidence, scrollToDetails],
  );

  return (
    <>
      {/* One banner landmark around the persistent chrome. The mode badge and
          the REPLAY line never scroll away; the provenance detail does, because
          "which recording" is a question you ask once and "is this live" is one
          you must never be able to get wrong. */}
      <header className="site-header" role="banner">
        <div className="chrome">
          <h1 className="brand">
            FleetForge <span>· incident replay</span>
          </h1>
          <ModeBadge mode={bundle.mode} label={bundle.modeLabel} />
          <span className="chrome-spacer" />
          <span className="meta chrome-meta">
            {context.context.cluster_kind} · {context.context.kubernetes_version ?? "version unknown"}
          </span>
        </div>

        <div className="banner banner-replay" role="note">
          <strong>REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET EXECUTION</strong>
          <span>
            Recorded {context.context.captured_from.slice(0, 10)} from a cluster that has since
            been destroyed. Nothing on this screen is live.
          </span>
        </div>
      </header>

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
        <ReplayOverview
          context={context.context}
          pdb={bundle.pdb}
          finalState={finalState}
          nodeCount={context.context.nodes.length}
          onExplore={() => goToDetails("timeline")}
        />

        <div ref={detailsRef}>
          {/* Position and provenance belong with the timeline, not with the
              story — they answer "where am I in the recording", which only
              matters once you are scrubbing it. */}
          <PositionStrip
            stale={playback.stale}
            state={playback.state}
            firstObservationAt={bundle.timeline.find((e) => e.kind === "node_changed")?.at}
          />
          <ProvenanceStrip
            context={context}
            bottlerocket={context.context.bottlerocket_versions}
            artifacts={bundle.artifacts.length}
            state={playback.state}
            stale={playback.stale}
            step={playback.step}
            steps={bundle.timeline.length}
          />

          <ReplayDetails
            timeline={bundle.timeline}
            chapters={bundle.chapters}
            playback={playback}
            totalEvents={context.events}
            chain={bundle.chain}
            claims={bundle.claims}
            pdb={bundle.pdb}
            predictions={bundle.predictions}
            traffic={bundle.traffic}
            artifacts={bundle.artifacts}
            caveats={context.caveats}
            capturedFrom={context.context.captured_from}
            brupopFirstSeenAt={context.context.brupop_first_seen_at}
            openArtifact={openArtifact}
            onOpenEvidence={openEvidenceFromAnywhere}
            activeTab={tab}
            onTab={setTab}
          />
        </div>
      </main>
    </>
  );
}

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

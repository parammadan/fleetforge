// Everything below the fold, in tabs.
//
// This is the proof. It is organised by the question a reader arrives with
// rather than by which crate produced it:
//
//   Timeline       what happened, minute by minute
//   Investigation  why it happened, and who worked that out
//   The finding    the arithmetic, and how the prediction scored
//   Evidence       the artifacts, with their hashes
//   Limits         what this cannot tell you
//
// Tabs rather than one long scroll: the previous landing page was a single
// column of eight panels, and a reader could not tell which of them answered
// their question without reading all of them. Nothing has been removed.

import { useId, useRef } from "react";

import { GlossaryList } from "./glossary";
import { InvestigationChainPanel } from "./InvestigationChain";
import {
  ClaimList,
  EvidenceExplorer,
  FleetTopology,
  Limitations,
  Panel,
  PredictedVsActual,
  PreflightFinding,
} from "./ReplayPanels";
import { ReplayTimeline } from "./ReplayTimeline";
import type { ArtifactRef, Chapter, Claim, DataCaveat, InvestigationChain } from "./replayTypes";
import type { PdbArithmetic, PredictionRow, ReplayEvent, TrafficValidation } from "./replayTypes";
import type { Playback } from "./useReplay";

const TABS = [
  { id: "timeline", label: "Timeline", hint: "Minute by minute" },
  { id: "investigation", label: "Investigation", hint: "Why it happened" },
  { id: "finding", label: "The finding", hint: "Arithmetic and scoring" },
  { id: "evidence", label: "Evidence", hint: "Artifacts and hashes" },
  { id: "limits", label: "Limits", hint: "What this cannot tell you" },
] as const;

type TabId = (typeof TABS)[number]["id"];

export interface DetailsProps {
  timeline: ReplayEvent[];
  chapters: Chapter[];
  playback: Playback;
  totalEvents: number;
  chain: InvestigationChain;
  claims: Claim[];
  pdb: PdbArithmetic;
  predictions: PredictionRow[];
  traffic: TrafficValidation;
  artifacts: ArtifactRef[];
  caveats: DataCaveat[];
  capturedFrom: string;
  brupopFirstSeenAt: string | null;
  openArtifact: string | null;
  onOpenEvidence: (name: string | null) => void;
  /** Set when a claim elsewhere opens an artifact, so the tab can follow. */
  activeTab: TabId;
  onTab: (tab: TabId) => void;
}

export function ReplayDetails(props: DetailsProps) {
  const {
    timeline,
    chapters,
    playback,
    totalEvents,
    chain,
    claims,
    pdb,
    predictions,
    traffic,
    artifacts,
    caveats,
    capturedFrom,
    brupopFirstSeenAt,
    openArtifact,
    onOpenEvidence,
    activeTab,
    onTab,
  } = props;

  const base = useId();
  const tabsRef = useRef<HTMLDivElement>(null);

  // Arrow keys move between tabs, as the ARIA tabs pattern requires. Without
  // it a keyboard user has to tab through every control in the open panel to
  // reach the next tab.
  const onKeyDown = (event: React.KeyboardEvent) => {
    const i = TABS.findIndex((t) => t.id === activeTab);
    const go = (next: number) => {
      event.preventDefault();
      const target = TABS[(next + TABS.length) % TABS.length];
      if (!target) return;
      onTab(target.id);
      tabsRef.current?.querySelector<HTMLButtonElement>(`#${base}-${target.id}`)?.focus();
    };
    if (event.key === "ArrowRight") go(i + 1);
    if (event.key === "ArrowLeft") go(i - 1);
    if (event.key === "Home") go(0);
    if (event.key === "End") go(TABS.length - 1);
  };

  return (
    <section className="details" id="details" aria-label="Incident detail">
      <div className="tabs" role="tablist" aria-label="Incident detail" ref={tabsRef} onKeyDown={onKeyDown}>
        {TABS.map((tab) => (
          <button
            key={tab.id}
            id={`${base}-${tab.id}`}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.id}
            aria-controls={`${base}-${tab.id}-panel`}
            tabIndex={activeTab === tab.id ? 0 : -1}
            className={`tab ${activeTab === tab.id ? "tab-active" : ""}`}
            onClick={() => onTab(tab.id)}
          >
            <span className="tab-label">{tab.label}</span>
            <span className="tab-hint">{tab.hint}</span>
          </button>
        ))}
      </div>

      <div
        role="tabpanel"
        id={`${base}-${activeTab}-panel`}
        aria-labelledby={`${base}-${activeTab}`}
        tabIndex={-1}
        className="tabpanel"
      >
        {activeTab === "timeline" && (
          <>
            <ReplayTimeline
              timeline={timeline}
              chapters={chapters}
              playback={playback}
              totalEvents={totalEvents}
            />
            <FleetTopology state={playback.state} onOpenEvidence={onOpenEvidence} />
          </>
        )}

        {activeTab === "investigation" && (
          <>
            <InvestigationChainPanel chain={chain} onOpenEvidence={onOpenEvidence} />
            <Panel
              id="claims"
              title="What FleetForge established, and how"
              subtitle={`${claims.length} claims · ${
                claims.filter((c) => c.basis === "observed_by_fleet_forge" || c.basis === "mathematically_derived").length
              } are evidence`}
            >
              <p className="claim-evidence">
                Every statement carries where it came from. <b>OBSERVED</b> means FleetForge
                watched it happen. <b>DERIVED</b> means it is arithmetic you can check.{" "}
                <b>HUMAN RCA</b> means a person worked it out afterwards. <b>UNVERIFIED</b>{" "}
                means plausible but never tested. <b>NO EVIDENCE</b> means the data does not
                exist. Only the first two are evidence.
              </p>
              <ClaimList claims={claims} onOpenEvidence={onOpenEvidence} />
            </Panel>
          </>
        )}

        {activeTab === "finding" && (
          <>
            <PreflightFinding pdb={pdb} preflight={playback.state?.last_preflight ?? null} />
            <PredictedVsActual rows={predictions} />
          </>
        )}

        {activeTab === "evidence" && (
          <EvidenceExplorer
            artifacts={artifacts}
            open={openArtifact}
            onOpen={onOpenEvidence}
          />
        )}

        {activeTab === "limits" && (
          <>
            <Limitations
              caveats={caveats}
              claims={claims}
              capturedFrom={capturedFrom}
              brupopFirstSeenAt={brupopFirstSeenAt}
            />
            <Panel id="traffic" title="The post-recovery networking check">
              <p>{traffic.interpretation}</p>
              <table className="summary">
                <tbody>
                  <tr>
                    <th scope="row">Result</th>
                    <td>
                      <strong className={traffic.passed ? "" : "value-bad"}>
                        {traffic.passed ? "PASSED" : "FAILED"}
                      </strong>
                    </td>
                  </tr>
                  <tr>
                    <th scope="row">Requests</th>
                    <td className="mono">
                      {traffic.successes} of {traffic.requests} succeeded ({traffic.success_pct}%)
                    </td>
                  </tr>
                  <tr>
                    <th scope="row">Window</th>
                    <td className="mono">{traffic.window}</td>
                  </tr>
                </tbody>
              </table>
              <p className="claim-evidence">
                This is a networking validation run after recovery. It is <b>not</b> an
                availability measurement for the incident, and must never be presented as
                uptime.
              </p>
            </Panel>
            <Panel id="glossary" title="Terms used on this page">
              <GlossaryList />
            </Panel>
          </>
        )}
      </div>
    </section>
  );
}

export type { TabId };
export { TABS };

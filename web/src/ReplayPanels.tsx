// Panels of the replay control room.
//
// Split from the layout so each one can be tested against a fixed prop shape
// without standing up the whole screen.

import { useEffect, useState } from "react";
import type {
  ArtifactContent,
  ArtifactRef,
  Claim,
  ClaimBasis,
  DataCaveat,
  NodeReplayState,
  PdbArithmetic,
  PredictionRow,
  ReplayState,
  TrafficValidation,
} from "./replayTypes";
import { BASIS_LABEL, BASIS_MEANING, isEvidence } from "./replayTypes";
import { Term } from "./glossary";
import { fetchArtifact } from "./useReplay";

/* ---------------------------------------------------------------- primitives */

/**
 * The label that makes the whole interface honest.
 *
 * Rendered next to every statement, never suppressed, and always carrying its
 * meaning as a tooltip. Colour distinguishes the two evidence kinds from the
 * three that are not evidence, but the word is what actually carries it.
 */
export function BasisTag({ basis }: { basis: ClaimBasis }) {
  return (
    <span
      className={`basis basis-${basis}`}
      title={BASIS_MEANING[basis]}
      data-testid={`basis-${basis}`}
    >
      {BASIS_LABEL[basis]}
    </span>
  );
}

export function Panel({
  title,
  subtitle,
  children,
  id,
}: {
  title: string;
  subtitle?: React.ReactNode;
  children: React.ReactNode;
  id?: string;
}) {
  return (
    <section className="panel" id={id}>
      <header>
        <h2>{title}</h2>
        {subtitle && <span className="count">{subtitle}</span>}
      </header>
      {/* Focusable because it scrolls horizontally: node tables carry
          resourceVersions and quantities that exceed a narrow column, and a
          scrollable region with no keyboard access is a region a keyboard user
          cannot read the right-hand side of. */}
      <div className="panel-body" tabIndex={0}>
        {children}
      </div>
    </section>
  );
}

function timeOf(iso: string): string {
  return iso.slice(11, 19);
}

/* ---------------------------------------------------------- executive summary */

export interface SummaryProps {
  capturedFrom: string;
  capturedTo: string;
  brupopFirstSeenAt: string | null;
  claims: Claim[];
  traffic: TrafficValidation;
  nodeCount: number;
  pdb: PdbArithmetic;
  onOpenEvidence?: (artifact: string) => void;
}

/**
 * What a leader needs in thirty seconds.
 *
 * The hardest thing on this screen is the availability tile. It is the number
 * every executive reaches for, and this incident does not have it: the sampler
 * running at the time had no request timeout, so its output measured how long
 * `wget` was willing to wait. The tile says UNKNOWN and explains why. It would
 * be trivial to put 30.2% there instead — that figure is a *failed
 * post-recovery networking check*, and showing it as uptime would be the single
 * most misleading thing this interface could do.
 */
export function ExecutiveSummary({
  capturedFrom,
  capturedTo,
  brupopFirstSeenAt,
  claims,
  traffic,
  nodeCount,
  pdb,
  onOpenEvidence,
}: SummaryProps) {
  const minutes = Math.round(
    (Date.parse(capturedTo) - Date.parse(capturedFrom)) / 60_000,
  );
  // Exact, and truncated rather than rounded — `chrono::Duration::num_seconds`
  // in the Rust chapter narration truncates, and a screen showing "7m 32s" in
  // one panel and "7m 33s" in another is a screen a careful reader is right to
  // distrust on both counts.
  const blindSeconds = brupopFirstSeenAt
    ? Math.floor((Date.parse(capturedFrom) - Date.parse(brupopFirstSeenAt)) / 1000)
    : null;
  const blindMinutes =
    blindSeconds === null
      ? null
      : `${Math.floor(blindSeconds / 60)}m ${blindSeconds % 60}s`;
  const evidenceClaims = claims.filter((c) => isEvidence(c.basis)).length;

  return (
    <Panel
      id="summary"
      title="Executive summary"
      subtitle={`${timeOf(capturedFrom)} → ${timeOf(capturedTo)} · ${minutes} min captured`}
    >
      <p className="headline">
        A routine <Term k="bottlerocket" /> update stalled. <Term k="brupop" /> cordoned two of
        three nodes, a <Term k="pdb">PodDisruptionBudget</Term> then refused every{" "}
        <Term k="eviction" />, and the update could not proceed. Two manual uncordons cleared
        it and all three nodes finished on Bottlerocket 1.64.0.
      </p>

      <div className="tiles">
        <div className="tile tile-unknown">
          <span className="tile-label">Customer availability</span>
          <strong className="tile-value">UNKNOWN DURING INCIDENT</strong>
          <span className="tile-note">
            The traffic sampler running at the time had no request timeout, so its output
            measured the client's patience rather than the service — 31 samples in 34 minutes
            instead of ~2,000. Its data was discarded. No availability figure exists for this
            window, in either direction.
          </span>
          <BasisTag basis="unavailable" />
        </div>

        <div className="tile tile-bad">
          <span className="tile-label">What was blocked</span>
          <strong className="tile-value">{pdb.finding_id}</strong>
          <span className="tile-note">
            {pdb.title}. {pdb.formula} = {pdb.result}, so the platform refused every voluntary
            pod removal. Affected: {pdb.affected.join(", ")}.
          </span>
          <BasisTag basis="mathematically_derived" />
        </div>

        <div className="tile">
          <span className="tile-label">Nodes updated</span>
          <strong className="tile-value">
            {nodeCount} / {nodeCount}
          </strong>
          <span className="tile-note">
            All nodes reached Bottlerocket 1.64.0. The update Brupop set out to perform
            completed.
          </span>
          <BasisTag basis="observed_by_fleet_forge" />
        </div>

        <div className="tile">
          <span className="tile-label">Recovery</span>
          <strong className="tile-value">2 manual uncordons</strong>
          <span className="tile-note">
            Both performed by a human with kubectl, at 14:35:11 and 14:53:06. Each restored
            scheduling progress. FleetForge holds no mutating client and issued nothing.
          </span>
          <BasisTag basis="observed_by_fleet_forge" />
        </div>

        <div className={`tile ${traffic.passed ? "" : "tile-bad"}`}>
          <span className="tile-label">Post-recovery networking check</span>
          <strong className="tile-value">
            {traffic.passed ? "PASSED" : "FAILED"} · {traffic.success_pct}%
          </strong>
          <span className="tile-note">{traffic.interpretation}</span>
          <BasisTag basis="observed_by_fleet_forge" />
        </div>

      </div>

      {/* A statement about the screen rather than about the incident, so it
          sits outside the tile grid — and stops a sixth tile orphaning onto a
          row of its own. */}
      <p className="trust-bar">
        <strong>Can you trust this?</strong> Every statement below carries where it came from.{" "}
        {evidenceClaims} of {claims.length} are measurements or arithmetic you can check.
        The other {claims.length - evidenceClaims} are human analysis, untested hypotheses, or
        explicitly unknown — labelled as such wherever they appear, including here.
      </p>

      {blindSeconds !== null && blindSeconds > 0 && (
        <p className="callout">
          <strong>FleetForge did not predict this.</strong> Brupop began updating the fleet
          at {timeOf(brupopFirstSeenAt ?? "")} — {blindMinutes} before FleetForge started
          recording at {timeOf(capturedFrom)}. Two nodes were already cordoned in the first
          state it ever saw. It detected the blocker that was in front of it; it did not
          foresee the deadlock, and nothing in this replay should be read as if it had.
        </p>
      )}

      <h3 className="section-heading">What FleetForge established, and how</h3>
      <ClaimList claims={claims} onOpenEvidence={onOpenEvidence} />
    </Panel>
  );
}

/* ------------------------------------------------------------ investigation */

export function ClaimList({
  claims,
  onOpenEvidence,
}: {
  claims: Claim[];
  onOpenEvidence?: (artifact: string) => void;
}) {
  return (
    <ol className="claims">
      {claims.map((claim) => (
        <li key={claim.id} className={`claim ${isEvidence(claim.basis) ? "" : "claim-soft"}`}>
          <div className="claim-head">
            <BasisTag basis={claim.basis} />
            {claim.at && <span className="mono claim-at">{timeOf(claim.at)}</span>}
          </div>
          <p className="claim-statement">{claim.statement}</p>
          {claim.limitations.length > 0 && (
            <ul className="limitations">
              {claim.limitations.map((l) => (
                <li key={l}>{l}</li>
              ))}
            </ul>
          )}
          {claim.evidence.length > 0 && (
            <p className="claim-evidence">
              Evidence:{" "}
              {claim.evidence.map((e, i) => (
                <span key={e}>
                  {i > 0 && ", "}
                  {onOpenEvidence ? (
                    <button
                      type="button"
                      className="link-button"
                      onClick={() => onOpenEvidence(e)}
                    >
                      {e}
                    </button>
                  ) : (
                    <code>{e}</code>
                  )}
                </span>
              ))}
            </p>
          )}
        </li>
      ))}
    </ol>
  );
}

/* ----------------------------------------------------------------- topology */

export function FleetTopology({
  state,
  onOpenEvidence,
}: {
  state: ReplayState | null;
  onOpenEvidence?: (artifact: string) => void;
}) {
  if (!state) {
    return (
      <Panel id="fleet" title="Fleet">
        <div className="state-block">
          <div className="spinner" aria-hidden="true" />
          <h3>Loading state for this position</h3>
          <p>
            Cluster state is computed by the Rust API, not by this browser. Nothing is drawn
            until it answers.
          </p>
        </div>
      </Panel>
    );
  }

  return (
    <Panel
      id="fleet"
      title="Fleet"
      subtitle={`${state.nodes.length} nodes · ${state.pods.length} pods · ${timeOf(state.at)}`}
    >
      <div className="fleet">
        {state.nodes.map((node) => (
          <NodeCard key={node.name} node={node} onOpenEvidence={onOpenEvidence} />
        ))}
      </div>
      <p className="fleet-footnote">
        {state.cordoned_nodes} cordoned · {state.pending_pods} pod
        {state.pending_pods === 1 ? "" : "s"} Pending · {state.events_applied} events applied
      </p>
    </Panel>
  );
}

/**
 * Where a node's own state can be checked, per phase of the capture.
 *
 * These are the artifacts that contain `kubectl get nodes -o json` at three
 * points: before the first uncordon, after it, and at the end. A viewer who
 * doubts what the card says can open the file the card was derived from.
 */
const NODE_EVIDENCE = [
  { name: "06-nodes-before.json", when: "before recovery" },
  { name: "14-nodes-after.json", when: "after first uncordon" },
  { name: "25-nodes-final.json", when: "final" },
];

function NodeCard({
  node,
  onOpenEvidence,
}: {
  node: NodeReplayState;
  onOpenEvidence?: (artifact: string) => void;
}) {
  const short = node.name.replace(/\.us-east-2\.compute\.internal$/, "");
  // 2.0.0 is the updater-interface-version label, read into the wrong field by
  // a bug fixed partway through the capture. Shown as recorded and flagged,
  // because correcting captured evidence in the renderer is how a replay stops
  // being one.
  const versionBug = node.bottlerocket_version === "2.0.0";

  return (
    <article className={`node-card ${node.unschedulable ? "node-cordoned" : ""}`}>
      <header>
        <h3>{short}</h3>
        {node.unschedulable ? (
          <span className="pill pill-danger">cordoned</span>
        ) : (
          <span className="pill pill-ok">schedulable</span>
        )}
        <span className={`pill ${node.ready ? "pill-ok" : "pill-danger"}`}>
          {node.ready ? "Ready" : "NotReady"}
        </span>
        <span className="chrome-spacer" />
        <span className={`mono ${versionBug ? "value-suspect" : ""}`} title={
          versionBug
            ? "Recorded as 2.0.0 by a field-mapping bug that was fixed partway through this capture. Shown as recorded."
            : undefined
        }>
          {node.bottlerocket_version ?? "version unknown"}
          {versionBug && " ⚠"}
        </span>
      </header>
      <div className="node-meta">
        <span>
          <Term k="brupop" /> state:{" "}
          <strong>{node.brupop_state ?? "no shadow observed"}</strong>
          {node.brupop_version && ` (${node.brupop_version})`}
        </span>
        <span>{node.pods.length} pods</span>
        {node.last_change && <span className="mono">changed {timeOf(node.last_change)}</span>}
      </div>
      <p className="node-transition">
        <span className={`transition ${transitionClass(node)}`}>{transitionOf(node)}</span>
        {onOpenEvidence && (
          <span className="node-evidence">
            evidence:{" "}
            {NODE_EVIDENCE.map((e, i) => (
              <span key={e.name}>
                {i > 0 && " · "}
                <button
                  type="button"
                  className="link-button"
                  onClick={() => onOpenEvidence(e.name)}
                  title={e.name}
                >
                  {e.when}
                </button>
              </span>
            ))}
          </span>
        )}
      </p>
      <ul className="pod-list">
        {node.pods.map((pod) => (
          <li key={pod} className="pod-chip pod-running">
            {pod.split("/")[1] ?? pod}
          </li>
        ))}
      </ul>
    </article>
  );
}

/* ---------------------------------------------------------------- preflight */

/**
 * FF-PDB-001, shown as arithmetic rather than as a verdict.
 *
 * The point of the panel is that an engineer can check it. Formula, inputs,
 * result, and the exact field each input was read from — a reader who disagrees
 * with the conclusion can find which number they disagree with.
 */
export function PreflightFinding({
  pdb,
  preflight,
}: {
  pdb: PdbArithmetic;
  preflight: ReplayState["last_preflight"];
}) {
  return (
    <Panel
      id="finding"
      title={`${pdb.finding_id} — ${pdb.title}`}
      subtitle={`${pdb.severity} · confidence: ${pdb.confidence}`}
    >
      <div className="verdict verdict-blocked">
        <strong>BLOCKED</strong>
        <span>
          Every eviction against this PodDisruptionBudget will be rejected while
          disruptionsAllowed is {pdb.result}.
        </span>
      </div>

      <h3 className="section-heading">The arithmetic</h3>
      <pre className="calc">{pdb.formula}</pre>
      <table className="summary">
        <tbody>
          {pdb.inputs.map(([name, value]) => (
            <tr key={name}>
              <th scope="row">{name}</th>
              <td className="mono">{value}</td>
            </tr>
          ))}
          <tr>
            <th scope="row">result</th>
            <td className="mono">
              <strong>
                {pdb.result} {pdb.unit}
              </strong>
            </td>
          </tr>
        </tbody>
      </table>

      <h3 className="section-heading">Where each number came from</h3>
      <table>
        <thead>
          <tr>
            <th>Resource</th>
            <th>Field</th>
            <th>Value</th>
            <th>Note</th>
          </tr>
        </thead>
        <tbody>
          {pdb.evidence.map((e) => (
            <tr key={`${e.resource}${e.field_path}`}>
              <td className="mono">{e.resource}</td>
              <td className="mono">{e.field_path}</td>
              <td className="mono">{e.value}</td>
              <td>{e.note ?? "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3 className="section-heading">What this finding does not tell you</h3>
      <ul className="limitations">
        {pdb.limitations.map((l) => (
          <li key={l}>{l}</li>
        ))}
      </ul>

      <p className="claim-evidence">
        Analyzed snapshot <code>{pdb.snapshot_id.slice(0, 12)}</code>
        {preflight && (
          <>
            {" "}· at this timeline position the most recent preflight ran at{" "}
            {timeOf(preflight.at)} and returned <strong>{preflight.status}</strong>, predicting{" "}
            {preflight.pods_evicted} evictions.
          </>
        )}
      </p>
    </Panel>
  );
}

/* -------------------------------------------------------- predicted vs actual */

const CLASS_LABEL: Record<PredictionRow["class"], string> = {
  exact: "exact",
  conservative: "conservative",
  under_predicted: "under-predicted",
  missed: "missed",
  untested: "untested",
};

export function PredictedVsActual({ rows }: { rows: PredictionRow[] }) {
  const scored = rows.filter((r) => r.class !== "untested");
  const under = rows.filter((r) => r.class === "under_predicted").length;

  return (
    <Panel
      id="predictions"
      title="Predicted versus actual"
      subtitle={`${rows.length} preflights · ${scored.length} scored`}
    >
      {under > 0 && (
        <div className="verdict verdict-blocked">
          <strong>{under} UNDER-PREDICTED</strong>
          <span>
            FleetForge predicted fewer evictions than actually occurred. This is the failure
            mode that matters: an operator who trusted the lower number would have been
            surprised. It is counted separately from "conservative" on purpose.
          </span>
        </div>
      )}

      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Node</th>
            <th>Predicted</th>
            <th>Observed</th>
            <th>Δ</th>
            <th>Disruption in window</th>
            <th>Verdict</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={`${r.at}${r.node_names.join()}`} className={`pred-${r.class}`}>
              <td className="mono">{timeOf(r.at)}</td>
              <td className="mono">
                {r.node_names
                  .map((n) => n.replace(/\.us-east-2\.compute\.internal$/, ""))
                  .join(", ")}
              </td>
              <td className="mono">{r.predicted}</td>
              <td className="mono">{r.observed}</td>
              <td className="mono">{r.delta > 0 ? `+${r.delta}` : r.delta}</td>
              <td>{r.window_had_activity ? "yes" : "none"}</td>
              <td>
                <span className={`pill pill-${pillFor(r.class)}`}>{CLASS_LABEL[r.class]}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <p className="claim-evidence">
        Two rows can both read "predicted 16, observed 0" and still score differently. When
        nothing at all was drained in the window the prediction was never put to the test and
        is scored <strong>untested</strong> — which is not a pass. When pods did move but none
        of the predicted ones did, the prediction was tested and over-shot, which is{" "}
        <strong>conservative</strong>. The "disruption in window" column is the difference.
      </p>
    </Panel>
  );
}

/**
 * Where this node is in its update, in words.
 *
 * Composed from Brupop's own reported state rather than inferred from node
 * conditions — Brupop is the thing performing the update, so its state is the
 * observation, and anything FleetForge deduced instead would be a second
 * opinion presented as a fact.
 */
function transitionOf(node: NodeReplayState): string {
  const s = node.brupop_state;
  if (!s) {
    // The most important case in this incident, and the easiest to render as a
    // shrug: a node held out of service with nothing saying why. Brupop
    // publishes its per-node state as a custom resource, and FleetForge had not
    // yet observed one for this node — which is a different statement from
    // "nothing is happening".
    return node.unschedulable
      ? "cordoned — no Brupop state observed for this node yet"
      : "no Brupop state observed for this node yet";
  }
  switch (s) {
    case "Idle":
      return node.unschedulable
        ? "Brupop idle — but this node is still cordoned"
        : "Brupop idle — nothing in flight";
    case "StagedAndPerformedUpdate":
      return "update staged and applied, waiting to reboot";
    case "RebootedIntoUpdate":
      return "rebooted into the new release, waiting to be uncordoned";
    case "MonitoringUpdate":
      return "rebooted, Brupop is checking health before finishing";
    default:
      return `Brupop reports ${s}`;
  }
}

function transitionClass(node: NodeReplayState): string {
  if (node.unschedulable) return "transition-stuck";
  if (node.brupop_state && node.brupop_state !== "Idle") return "transition-active";
  return "transition-done";
}

function pillFor(c: PredictionRow["class"]): string {
  if (c === "under_predicted" || c === "missed") return "danger";
  if (c === "exact") return "ok";
  if (c === "conservative") return "warn";
  return "dim";
}

/* -------------------------------------------------------- evidence explorer */

export function EvidenceExplorer({
  artifacts,
  open,
  onOpen,
}: {
  artifacts: ArtifactRef[];
  /** Controlled from above so a claim elsewhere on the page can open one. */
  open: string | null;
  onOpen: (name: string | null) => void;
}) {
  const [content, setContent] = useState<ArtifactContent | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setContent(null);
      return;
    }
    let cancelled = false;
    setContent(null);
    setError(null);
    fetchArtifact(open)
      .then((c) => {
        if (!cancelled) setContent(c);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  const total = artifacts.reduce((sum, a) => sum + a.bytes, 0);

  return (
    <Panel
      id="evidence"
      title="Evidence"
      subtitle={`${artifacts.length} artifacts · ${Math.round(total / 1024)} KiB`}
    >
      <p className="claim-evidence">
        Every artifact is addressed by name from a manifest, never by path, and is served
        through a redactor before it reaches this page. Each carries the SHA-256 it had when
        captured, so a reader can prove the file they are looking at is the file that was
        recorded.
      </p>
      <table>
        <thead>
          <tr>
            <th>Artifact</th>
            <th>Contents</th>
            <th>Size</th>
            <th>SHA-256</th>
          </tr>
        </thead>
        <tbody>
          {artifacts.map((a) => (
            <tr key={a.name}>
              <td>
                <button
                  type="button"
                  className="link-button"
                  onClick={() => onOpen(open === a.name ? null : a.name)}
                  aria-expanded={open === a.name}
                >
                  {a.name}
                </button>
              </td>
              <td>{a.description}</td>
              <td className="mono">{a.bytes.toLocaleString()}</td>
              <td className="mono" title={a.sha256}>
                {a.sha256.slice(0, 12)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {open && (
        <div className="drawer" id="evidence-drawer">
          <div className="drawer-head">
            <h4>{open}</h4>
            {content?.sha256 && (
              <span className="mono drawer-hash" title={`SHA-256: ${content.sha256}`}>
                sha256 {content.sha256.slice(0, 16)}…
              </span>
            )}
            <button type="button" className="link-button" onClick={() => onOpen(null)}>
              close
            </button>
          </div>
          {/* Human-readable description first; the raw bytes are below it and
              are what a reader falls back to, not what they start with. */}
          <p className="drawer-explanation">
            {artifacts.find((a) => a.name === open)?.description ?? "Captured artifact."}
          </p>
          {error && <p className="state-forbidden">{error}</p>}
          {!content && !error && <div className="spinner" aria-hidden="true" />}
          {content && (
            <details open>
              <summary>Raw {content.kind ?? "file"}</summary>
              <pre className="artifact" tabIndex={0} aria-label={`Raw contents of ${open}`}>
                {content.content}
              </pre>
            </details>
          )}
        </div>
      )}
    </Panel>
  );
}

/* ------------------------------------------------------------- limitations */

export function Limitations({
  caveats,
  claims,
  capturedFrom,
  brupopFirstSeenAt,
}: {
  caveats: DataCaveat[];
  claims: Claim[];
  capturedFrom?: string;
  brupopFirstSeenAt?: string | null;
}) {
  const soft = claims.filter((c) => !isEvidence(c.basis));
  // Truncated, to agree with the Rust-side narration. See ExecutiveSummary.
  const gap =
    capturedFrom && brupopFirstSeenAt
      ? Math.floor((Date.parse(capturedFrom) - Date.parse(brupopFirstSeenAt)) / 1000)
      : null;

  return (
    <Panel
      id="limitations"
      title="What this replay cannot tell you"
      subtitle={`${caveats.length} data caveats · ${soft.length} unverified or unknown claims`}
    >
      {/* The five disclosures that must survive a reader who skims. They are
          first, they are numbered, and each one says what it costs you. */}
      <ol className="disclosures">
        <li>
          <strong>Recording began after Brupop did.</strong>{" "}
          {gap === null ? (
            <>The bundle cannot establish when Brupop started, so whether this capture covers
            the beginning of the incident is unknown.</>
          ) : (
            <>
              Brupop started at {timeOf(brupopFirstSeenAt ?? "")}; FleetForge started recording
              at {timeOf(capturedFrom ?? "")}, {Math.floor(gap / 60)}m {gap % 60}s later.
              Everything before that point happened outside the evidence.
            </>
          )}
        </li>
        <li>
          <strong>No pre-update prediction exists.</strong> FleetForge was not running when the
          update began, so there is no preflight from before the first cordon. It cannot be
          shown to have predicted the deadlock, and this interface does not imply it.
        </li>
        <li>
          <strong>Original availability cannot be determined.</strong> The sampler in use had
          no request timeout and produced 31 samples in 34 minutes. Its output was discarded.
          No figure exists, high or low.
        </li>
        <li>
          <strong>The causal chain includes human analysis.</strong> The five facts were
          observed. The four arrows joining them were drawn by a person afterwards. FleetForge
          implements no analyzer that correlates cordons, scheduling and disruption budgets.
        </li>
        <li>
          <strong>The networking root cause is unverified.</strong> The add-on ordering
          hypothesis was never tested — no packet capture, no per-endpoint probe, no controlled
          comparison. The Terraform correction is written and not verified on a fresh cluster,
          because the cluster was destroyed first.
        </li>
      </ol>

      <h3 className="section-heading">Problems with the evidence itself</h3>
      <ul className="limitations">
        {caveats.map((c) => (
          <li key={c.id}>
            <strong>{c.id}</strong> — {c.statement}
            <br />
            <span className="mono">affects: {c.affects.join(", ")}</span>
          </li>
        ))}
      </ul>

      <h3 className="section-heading">Statements that are not evidence</h3>
      <ul className="limitations">
        {soft.map((c) => (
          <li key={c.id}>
            <BasisTag basis={c.basis} /> {c.statement}
          </li>
        ))}
      </ul>

      <h3 className="section-heading">Scope</h3>
      <ul className="limitations">
        <li>
          One cluster, three nodes, one capture, 63 minutes. Nothing here establishes how
          FleetForge behaves at fleet scale.
        </li>
        <li>
          FleetForge observed and explained. Brupop executed. No mutation was issued by
          FleetForge at any point, so nothing in this replay demonstrates safe execution —
          only safe analysis.
        </li>
        <li>
          The replay is deterministic over a fixed bundle. It proves what was recorded; it
          does not prove the recording was complete.
        </li>
      </ul>
    </Panel>
  );
}

export { timeOf };

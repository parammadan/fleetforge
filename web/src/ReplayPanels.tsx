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
      <div className="panel-body">{children}</div>
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
}: SummaryProps) {
  const minutes = Math.round(
    (Date.parse(capturedTo) - Date.parse(capturedFrom)) / 60_000,
  );
  const blindMinutes = brupopFirstSeenAt
    ? Math.round((Date.parse(capturedFrom) - Date.parse(brupopFirstSeenAt)) / 60_000)
    : null;

  return (
    <Panel
      id="summary"
      title="Executive summary"
      subtitle={`${timeOf(capturedFrom)} → ${timeOf(capturedTo)} · ${minutes} min captured`}
    >
      <div className="tiles">
        <div className="tile tile-unknown">
          <span className="tile-label">Availability during the incident</span>
          <strong className="tile-value">UNKNOWN</strong>
          <span className="tile-note">
            The traffic sampler running at the time had no request timeout, so its output
            measured the client's patience rather than the service. Its data was discarded.
            No availability figure exists for this window.
          </span>
          <BasisTag basis="unavailable" />
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
            Performed by a human. FleetForge has no execution path and issued no mutation at
            any point in this capture.
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

      {blindMinutes !== null && blindMinutes > 0 && (
        <p className="callout">
          <strong>FleetForge did not predict this.</strong> Brupop began updating the fleet
          at {timeOf(brupopFirstSeenAt ?? "")}, about {blindMinutes} minutes before FleetForge
          started recording at {timeOf(capturedFrom)}. Two nodes were already cordoned in the
          first state it ever saw. It detected the blocker that was in front of it; it did
          not foresee the deadlock, and nothing in this replay should be read as if it had.
        </p>
      )}

      <h3 className="section-heading">What FleetForge established, and how</h3>
      <ClaimList claims={claims} />
    </Panel>
  );
}

/* ------------------------------------------------------------ investigation */

export function ClaimList({ claims }: { claims: Claim[] }) {
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
                  <code>{e}</code>
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

export function FleetTopology({ state }: { state: ReplayState | null }) {
  if (!state) {
    return (
      <Panel title="Fleet">
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
      title="Fleet"
      subtitle={`${state.nodes.length} nodes · ${state.pods.length} pods · ${timeOf(state.at)}`}
    >
      <div className="fleet">
        {state.nodes.map((node) => (
          <NodeCard key={node.name} node={node} />
        ))}
      </div>
      <p className="fleet-footnote">
        {state.cordoned_nodes} cordoned · {state.pending_pods} pod
        {state.pending_pods === 1 ? "" : "s"} Pending · {state.events_applied} events applied
      </p>
    </Panel>
  );
}

function NodeCard({ node }: { node: NodeReplayState }) {
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
          Brupop: <strong>{node.brupop_state ?? "no shadow observed"}</strong>
          {node.brupop_version && ` (${node.brupop_version})`}
        </span>
        <span>{node.pods.length} pods</span>
      </div>
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

function pillFor(c: PredictionRow["class"]): string {
  if (c === "under_predicted" || c === "missed") return "danger";
  if (c === "exact") return "ok";
  if (c === "conservative") return "warn";
  return "dim";
}

/* -------------------------------------------------------- evidence explorer */

export function EvidenceExplorer({ artifacts }: { artifacts: ArtifactRef[] }) {
  const [open, setOpen] = useState<string | null>(null);
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
                  onClick={() => setOpen(open === a.name ? null : a.name)}
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
        <div className="drawer">
          <h4>{open}</h4>
          {error && <p className="state-forbidden">{error}</p>}
          {!content && !error && <div className="spinner" aria-hidden="true" />}
          {content && <pre className="artifact">{content.content}</pre>}
        </div>
      )}
    </Panel>
  );
}

/* ------------------------------------------------------------- limitations */

export function Limitations({
  caveats,
  claims,
}: {
  caveats: DataCaveat[];
  claims: Claim[];
}) {
  const soft = claims.filter((c) => !isEvidence(c.basis));

  return (
    <Panel
      id="limitations"
      title="What this replay cannot tell you"
      subtitle={`${caveats.length} data caveats · ${soft.length} unverified or unknown claims`}
    >
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

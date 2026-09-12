// The preflight workspace.
//
// Select nodes, set a concurrency, see the risk recompute. The evidence drawer
// is not an afterthought: a finding the operator cannot check is a finding they
// have to take on faith, which is the opposite of what this product is for.

import { useCallback, useEffect, useState } from "react";
import type { NodeFact } from "./types";
import { formatBytes, formatMillicores } from "./types";
import type { Finding, PreflightOutcome } from "./preflight";
import { bySeverity, confidenceText, runPreflight, severityClass } from "./preflight";

function EvidenceDrawer({ finding }: { finding: Finding }) {
  return (
    <div className="drawer">
      <p className="drawer-explanation">{finding.explanation}</p>

      <div className="drawer-meta">
        <span className={`pill ${severityClass(finding.severity)}`}>{finding.severity}</span>
        <span className="pill pill-dim">{confidenceText(finding.confidence)}</span>
        <span className="meta">analyzed snapshot {finding.snapshot_id.slice(0, 12)}</span>
      </div>

      {finding.evidence.length > 0 && (
        <>
          <h4>Evidence</h4>
          <table>
            <thead>
              <tr>
                <th scope="col">Object</th>
                <th scope="col">Field</th>
                <th scope="col">Value</th>
              </tr>
            </thead>
            <tbody>
              {finding.evidence.map((e, i) => (
                <tr key={`${e.field_path}-${i}`}>
                  <td className="mono">
                    {e.resource.kind}/{e.resource.name}
                    {e.resource.resource_version && (
                      <span className="meta"> @{e.resource.resource_version}</span>
                    )}
                  </td>
                  <td className="mono">{e.field_path}</td>
                  <td className="mono">
                    <strong>{e.value}</strong>
                    {e.note && <div className="meta">{e.note}</div>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {finding.calculation && (
        <>
          <h4>Calculation</h4>
          <pre className="calc">
            {finding.calculation.inputs.map(([k, v]) => `${k} = ${v}\n`).join("")}
            {"\n"}
            {finding.calculation.formula}
            {"\n=> "}
            {finding.calculation.result}
            {finding.calculation.unit ? ` ${finding.calculation.unit}` : ""}
          </pre>
        </>
      )}

      {finding.remediation.length > 0 && (
        <>
          <h4>Suggested remediation</h4>
          <ul className="remediation">
            {finding.remediation.map((r, i) => (
              <li key={i}>
                {r.description}
                {r.command && <pre className="calc">{r.command}</pre>}
                {r.tradeoff && <div className="meta">Trade-off: {r.tradeoff}</div>}
              </li>
            ))}
          </ul>
          <p className="meta">
            FleetForge does not apply these. It holds no mutating Kubernetes client.
          </p>
        </>
      )}

      <h4>Limitations — what this check does not prove</h4>
      <ul className="limitations">
        {finding.limitations.map((l, i) => (
          <li key={i}>{l}</li>
        ))}
      </ul>
    </div>
  );
}

function FindingRow({ finding }: { finding: Finding }) {
  const [open, setOpen] = useState(finding.severity === "blocker");
  return (
    <li className={`finding finding-${finding.severity}`}>
      <button
        type="button"
        className="finding-header"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span className={`pill ${severityClass(finding.severity)}`}>{finding.severity}</span>
        <code className="finding-id">{finding.id}</code>
        <span className="finding-title">{finding.title}</span>
        <span className="finding-toggle" aria-hidden="true">
          {open ? "Hide evidence" : "Show evidence"}
        </span>
      </button>
      {open && <EvidenceDrawer finding={finding} />}
    </li>
  );
}

export function PreflightWorkspace({ nodes }: { nodes: NodeFact[] }) {
  const [selected, setSelected] = useState<string[]>([]);
  const [concurrency, setConcurrency] = useState(1);
  const [outcome, setOutcome] = useState<PreflightOutcome>({ kind: "idle" });

  const toggle = useCallback((name: string) => {
    setSelected((current) =>
      current.includes(name) ? current.filter((n) => n !== name) : [...current, name],
    );
  }, []);

  useEffect(() => {
    if (selected.length === 0) {
      setOutcome({ kind: "idle" });
      return;
    }
    let cancelled = false;
    setOutcome({ kind: "running" });
    // Small debounce so dragging the concurrency control does not fire a
    // request per step.
    const timer = window.setTimeout(() => {
      void runPreflight(selected, concurrency).then((result) => {
        if (!cancelled) setOutcome(result);
      });
    }, 150);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [selected, concurrency]);

  const maxConcurrency = Math.max(1, selected.length);

  return (
    <section className="panel">
      <header>
        <h2>Preflight workspace</h2>
        <span className="count">
          {selected.length} node{selected.length === 1 ? "" : "s"} selected
        </span>
        {outcome.kind === "ok" && <span className="mode-badge mode-what_if">WHAT-IF</span>}
      </header>

      <div className="panel-body">
        <fieldset className="node-picker">
          <legend>Select nodes for maintenance</legend>
          {nodes.map((n) => (
            <label key={n.name} className="node-option">
              <input
                type="checkbox"
                checked={selected.includes(n.name)}
                onChange={() => toggle(n.name)}
              />
              <span className="mono">{n.name}</span>
              <span className="meta">
                {formatMillicores(n.allocatable_cpu)} / {formatBytes(n.allocatable_memory)}
                {n.unschedulable && " · cordoned"}
              </span>
            </label>
          ))}
        </fieldset>

        <div className="concurrency">
          <label htmlFor="concurrency">
            Nodes out of service at once
            <span className="meta">
              {" "}
              — the same selection can be safe at 1 and blocked at 2
            </span>
          </label>
          <div className="concurrency-control">
            <input
              id="concurrency"
              type="range"
              min={1}
              max={maxConcurrency}
              value={Math.min(concurrency, maxConcurrency)}
              disabled={selected.length === 0}
              onChange={(e) => setConcurrency(Number(e.target.value))}
            />
            <output className="mono">{Math.min(concurrency, maxConcurrency)}</output>
          </div>
        </div>

        {outcome.kind === "idle" && (
          <div className="state-block">
            <h3>No nodes selected</h3>
            <p>Choose one or more nodes above to analyze a maintenance operation.</p>
          </div>
        )}

        {outcome.kind === "running" && (
          <div className="state-block">
            <div className="spinner" aria-hidden="true" />
            <h3>Analyzing</h3>
          </div>
        )}

        {outcome.kind === "error" && (
          <div className="state-block state-forbidden">
            <h3>Preflight could not run</h3>
            <p>{outcome.error.message}</p>
            {outcome.error.retriable && <p className="meta">This may succeed if retried.</p>}
          </div>
        )}

        {outcome.kind === "ok" && (
          <>
            <div
              className={`verdict verdict-${outcome.result.summary.status}`}
              role="status"
              aria-live="polite"
            >
              <strong>{outcome.result.summary.status === "safe" ? "SAFE" : "BLOCKED"}</strong>
              <span>
                {outcome.result.summary.status === "safe"
                  ? "No blocker was found. This is not a promise that nothing will go wrong — each check below states what it does not prove."
                  : "At least one blocker applies. This maintenance must not proceed as selected."}
              </span>
            </div>

            {!outcome.authoritative && (
              <div className="banner banner-danger">
                <strong>Incomplete inputs</strong>
                <span>
                  Some cluster state could not be collected, so this analysis is missing checks.
                  Its silence about those is not a pass.
                </span>
              </div>
            )}

            <table className="summary">
              <tbody>
                <tr>
                  <th scope="row">Recommended concurrency</th>
                  <td className="mono">
                    <strong>{outcome.result.summary.recommended_max_concurrency}</strong>
                    {outcome.result.summary.concurrency_constraint && (
                      <div className="meta">
                        because {outcome.result.summary.concurrency_constraint.reason} (
                        {outcome.result.summary.concurrency_constraint.finding_id})
                      </div>
                    )}
                  </td>
                </tr>
                <tr>
                  <th scope="row">Pods evicted</th>
                  <td className="mono">
                    {outcome.result.summary.predicted_impact.pods_evicted}
                    {outcome.result.summary.predicted_impact.pods_not_rescheduled > 0 && (
                      <span className="meta">
                        {" "}
                        · {outcome.result.summary.predicted_impact.pods_not_rescheduled} not
                        rescheduled elsewhere (DaemonSet)
                      </span>
                    )}
                  </td>
                </tr>
                <tr>
                  <th scope="row">Must be absorbed</th>
                  <td className="mono">
                    {formatMillicores(outcome.result.summary.predicted_impact.cpu_to_reschedule)} CPU
                    {" · "}
                    {formatBytes(outcome.result.summary.predicted_impact.memory_to_reschedule)} memory
                  </td>
                </tr>
                <tr>
                  <th scope="row">Headroom after</th>
                  <td className="mono">
                    {formatMillicores(outcome.result.summary.predicted_impact.cpu_headroom_after)} CPU
                    {" · "}
                    {formatBytes(outcome.result.summary.predicted_impact.memory_headroom_after)} memory
                  </td>
                </tr>
                <tr>
                  <th scope="row">Smallest PDB margin</th>
                  <td className="mono">
                    {outcome.result.summary.predicted_impact.minimum_pdb_margin}
                  </td>
                </tr>
              </tbody>
            </table>

            {outcome.result.summary.affected_workloads.length > 0 && (
              <>
                <h3 className="section-heading">Affected workloads</h3>
                <table>
                  <thead>
                    <tr>
                      <th scope="col">Workload</th>
                      <th scope="col">Pods on selected nodes</th>
                      <th scope="col">Ready</th>
                      <th scope="col">Flags</th>
                    </tr>
                  </thead>
                  <tbody>
                    {outcome.result.summary.affected_workloads.map((w) => (
                      <tr key={`${w.workload.namespace}/${w.workload.name}`}>
                        <th scope="row" className="mono" style={{ fontWeight: 500 }}>
                          {w.workload.namespace}/{w.workload.name}
                        </th>
                        <td className="mono">{w.pods_on_selected_nodes}</td>
                        <td className="mono">
                          {w.ready_replicas ?? 0}/{w.desired_replicas ?? 0}
                        </td>
                        <td>
                          {w.blocked_by_pdb && <span className="pill pill-danger">PDB blocks</span>}
                          {w.has_immovable_pods && (
                            <span className="pill pill-danger">node-bound storage</span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </>
            )}

            <h3 className="section-heading">
              Findings <span className="count">{outcome.result.findings.length}</span>
            </h3>
            <ul className="findings">
              {[...outcome.result.findings].sort(bySeverity).map((f) => (
                <FindingRow key={f.id} finding={f} />
              ))}
            </ul>
          </>
        )}
      </div>
    </section>
  );
}

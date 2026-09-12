// Brupop state and the prediction-versus-actual view.
//
// The accuracy section leads with the misses. A tool reporting only its hits is
// marking its own homework, and "nothing was disrupted, so every prediction
// matched" is the specific way that goes wrong — so an untested prediction is
// shown as untested, never as correct.

import { useCallback, useEffect, useState } from "react";
import type { BrupopFact, CollectionStatus } from "./types";
import { EmptyState, StatusPill } from "./components";
import { isAuthoritative } from "./types";

interface WorkloadKey {
  namespace: string;
  name: string;
}

interface Comparison {
  predicted_at: string;
  snapshot_id: string;
  node_names: string[];
  prediction: { pods_evicted: number; status: string };
  observed_evictions: number;
  observed_but_not_predicted: WorkloadKey[];
  predicted_but_not_observed: WorkloadKey[];
  window_had_activity: boolean;
}

interface Accuracy {
  predictions: number;
  tested: number;
  exact: number;
  conservative: number;
  missed: number;
}

interface TimelineItem {
  seq: number;
  at: string;
  kind: string;
  detail: string;
}

interface Report {
  run_id: string;
  description: string | null;
  entries: number;
  comparisons: Comparison[];
  accuracy: Accuracy;
  timeline: TimelineItem[];
  caveats: string[];
}

function verdict(c: Comparison): { text: string; cls: string } {
  if (!c.window_had_activity) {
    return { text: "untested", cls: "pill-dim" };
  }
  if (c.observed_but_not_predicted.length > 0) {
    return { text: "missed", cls: "pill-danger" };
  }
  if (c.predicted_but_not_observed.length > 0 || c.observed_evictions !== c.prediction.pods_evicted) {
    return { text: "conservative", cls: "pill-warn" };
  }
  return { text: "exact", cls: "pill-ok" };
}

export function BrupopPanel({
  shadows,
  status,
}: {
  shadows: BrupopFact[];
  status: CollectionStatus | undefined;
}) {
  return (
    <section className="panel">
      <header>
        <h2>Brupop</h2>
        <span className="count">{shadows.length}</span>
        {status && !isAuthoritative(status) && <StatusPill status={status} />}
      </header>
      {shadows.length === 0 ? (
        <EmptyState kind="BottlerocketShadows" status={status} />
      ) : (
        <div className="panel-body">
          <table>
            <thead>
              <tr>
                <th scope="col">Node</th>
                <th scope="col">State</th>
                <th scope="col">Version</th>
              </tr>
            </thead>
            <tbody>
              {shadows.map((s) => {
                const moving =
                  s.current_state !== null &&
                  s.target_state !== null &&
                  s.current_state !== s.target_state;
                return (
                  <tr key={s.node_name}>
                    <th scope="row" className="mono" style={{ fontWeight: 500 }}>
                      {s.node_name}
                    </th>
                    <td className="mono">
                      {s.current_state ?? "—"}
                      {moving && (
                        <>
                          {" → "}
                          <strong>{s.target_state}</strong>
                          <span className="pill pill-warn" style={{ marginLeft: 8 }}>
                            updating
                          </span>
                        </>
                      )}
                    </td>
                    <td className="mono">
                      {s.current_version ?? "—"}
                      {s.target_version && s.target_version !== s.current_version && (
                        <> → {s.target_version}</>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <p className="meta">
            FleetForge reads Brupop's state. Brupop is the executor; FleetForge never writes here.
          </p>
        </div>
      )}
    </section>
  );
}

export function ReportPanel() {
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const response = await fetch("/api/v1/report");
      if (response.status === 404) {
        const body = (await response.json()) as { message: string };
        setError(body.message);
        return;
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      setReport((await response.json()) as Report);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void load();
    // Refreshed on the same cadence as the heartbeat; the log only grows.
    const timer = window.setInterval(() => void load(), 10_000);
    return () => window.clearInterval(timer);
  }, [load]);

  if (error) {
    return (
      <section className="panel">
        <header>
          <h2>Prediction versus actual</h2>
        </header>
        <div className="state-block">
          <h3>Not recording</h3>
          <p>{error}</p>
          <p className="meta">
            Start FleetForge with <code>--record PATH</code> to produce an event log.
          </p>
        </div>
      </section>
    );
  }

  if (!report) {
    return (
      <section className="panel">
        <header>
          <h2>Prediction versus actual</h2>
        </header>
        <div className="state-block">
          <div className="spinner" aria-hidden="true" />
          <h3>Loading the report</h3>
        </div>
      </section>
    );
  }

  const measured = report.accuracy.tested > 0;

  return (
    <section className="panel">
      <header>
        <h2>Prediction versus actual</h2>
        <span className="count mono">
          {report.run_id} · {report.entries} log entries
        </span>
        <a className="meta" href="/api/v1/report?format=markdown">
          export report
        </a>
      </header>
      <div className="panel-body">
        {!measured ? (
          <div className="banner banner-warn" style={{ border: "1px solid", borderRadius: 6 }}>
            <strong>Unmeasured</strong>
            <span>
              {report.accuracy.predictions} prediction(s) recorded, none tested — no disruption
              occurred during this run. That is not the same as being correct.
            </span>
          </div>
        ) : (
          <table className="summary">
            <tbody>
              <tr>
                <th scope="row">Tested predictions</th>
                <td className="mono">{report.accuracy.tested}</td>
              </tr>
              <tr>
                <th scope="row">Exact</th>
                <td className="mono">{report.accuracy.exact}</td>
              </tr>
              <tr>
                <th scope="row">Conservative</th>
                <td className="mono">{report.accuracy.conservative}</td>
              </tr>
              <tr>
                <th scope="row">Missed disruption</th>
                <td className="mono">
                  <strong className={report.accuracy.missed > 0 ? "" : undefined}>
                    {report.accuracy.missed}
                  </strong>
                  {report.accuracy.missed > 0 && (
                    <span className="pill pill-danger" style={{ marginLeft: 8 }}>
                      disruption occurred that was not predicted
                    </span>
                  )}
                </td>
              </tr>
            </tbody>
          </table>
        )}

        {report.comparisons.length > 0 && (
          <>
            <h3 className="section-heading">Predictions</h3>
            <table>
              <thead>
                <tr>
                  <th scope="col">Made at</th>
                  <th scope="col">Nodes</th>
                  <th scope="col">Predicted</th>
                  <th scope="col">Observed</th>
                  <th scope="col">Verdict</th>
                </tr>
              </thead>
              <tbody>
                {report.comparisons.map((c, i) => {
                  const v = verdict(c);
                  return (
                    <tr key={i}>
                      <td className="mono">{new Date(c.predicted_at).toLocaleTimeString()}</td>
                      <td className="mono">{c.node_names.join(", ")}</td>
                      <td className="mono">{c.prediction.pods_evicted}</td>
                      <td className="mono">{c.observed_evictions}</td>
                      <td>
                        <span className={`pill ${v.cls}`}>{v.text}</span>
                        {c.observed_but_not_predicted.length > 0 && (
                          <div className="meta">
                            not predicted:{" "}
                            {c.observed_but_not_predicted
                              .map((w) => `${w.namespace}/${w.name}`)
                              .join(", ")}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </>
        )}

        <h3 className="section-heading">Timeline</h3>
        <table>
          <thead>
            <tr>
              <th scope="col">seq</th>
              <th scope="col">Time</th>
              <th scope="col">Kind</th>
              <th scope="col">Detail</th>
            </tr>
          </thead>
          <tbody>
            {report.timeline.slice(-25).map((item) => (
              <tr key={item.seq}>
                <td className="mono">{item.seq}</td>
                <td className="mono">{new Date(item.at).toLocaleTimeString()}</td>
                <td className="mono">{item.kind}</td>
                <td>{item.detail}</td>
              </tr>
            ))}
          </tbody>
        </table>

        {report.caveats.length > 0 && (
          <>
            <h3 className="section-heading">Caveats</h3>
            <ul className="limitations">
              {report.caveats.map((c, i) => (
                <li key={i}>{c}</li>
              ))}
            </ul>
          </>
        )}
      </div>
    </section>
  );
}

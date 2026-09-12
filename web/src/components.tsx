// Presentational pieces.
//
// The recurring idea: absence is never rendered as "none" unless the backend
// said the kind was collected authoritatively. Every panel that can show an
// empty list therefore takes its CollectionStatus and decides between four
// different empty-looking outcomes: syncing, forbidden, degraded, or genuinely
// empty (ADR-0019).

import type { CollectionStatus, EventFact, KindCoverage, NodeFact, PdbFact, PodFact,
  StreamState, WorkloadFact } from "./types";
import { formatBytes, formatMillicores, isAuthoritative, statusLabel } from "./types";

export function ModeBadge({ mode, label }: { mode: string; label: string }) {
  return (
    <span className={`mode-badge mode-${mode}`} title={`Data mode: ${label}`}>
      {label}
    </span>
  );
}

export function StreamIndicator({ stream }: { stream: StreamState }) {
  const text =
    stream.kind === "open"
      ? "live stream connected"
      : stream.kind === "connecting"
        ? "connecting"
        : stream.kind === "stale"
          ? `stale — no update for ${Math.round((Date.now() - stream.lastEventAt) / 1000)}s`
          : "disconnected — retrying";
  return (
    <span className="stream-state" role="status" aria-live="polite">
      <span className={`dot dot-${stream.kind}`} aria-hidden="true" />
      {text}
    </span>
  );
}

export function StatusPill({ status }: { status: CollectionStatus }) {
  const cls =
    status.state === "in_sync"
      ? "pill-ok"
      : status.state === "forbidden" || status.state === "degraded"
        ? "pill-danger"
        : status.state === "stale"
          ? "pill-warn"
          : "pill-dim";
  return <span className={`pill ${cls}`}>{statusLabel(status)}</span>;
}

/**
 * Decide what to render when a list is empty.
 *
 * This is the component that keeps the product honest. "No PodDisruptionBudgets"
 * and "not allowed to list PodDisruptionBudgets" look the same in the data and
 * must never look the same on screen.
 */
export function EmptyState({
  kind,
  status,
}: {
  kind: string;
  status: CollectionStatus | undefined;
}) {
  if (!status || status.state === "syncing") {
    return (
      <div className="state-block">
        <div className="spinner" aria-hidden="true" />
        <h3>Syncing {kind}</h3>
        <p>The first watch has not completed. This is not an empty cluster.</p>
      </div>
    );
  }
  if (status.state === "forbidden") {
    return (
      <div className="state-block state-forbidden">
        <h3>Not permitted to read {kind}</h3>
        <p>
          RBAC denies <code>{status.verb}</code> on <code>{status.resource}</code>. FleetForge
          cannot see these objects, so it is not claiming there are none.
        </p>
      </div>
    );
  }
  if (status.state === "degraded") {
    return (
      <div className="state-block state-forbidden">
        <h3>{kind} collection is degraded</h3>
        <p>{status.error} — the list below may be incomplete or out of date.</p>
      </div>
    );
  }
  if (status.state === "stale") {
    return (
      <div className="state-block">
        <h3>{kind} data is stale</h3>
        <p>Last confirmed current {status.age_seconds}s ago.</p>
      </div>
    );
  }
  return (
    <div className="state-block">
      <h3>No {kind}</h3>
      <p>The watch is current and this cluster genuinely has none.</p>
    </div>
  );
}

function Panel({
  title,
  count,
  status,
  children,
}: {
  title: string;
  count?: number;
  status?: CollectionStatus;
  children: React.ReactNode;
}) {
  return (
    <section className="panel">
      <header>
        <h2>{title}</h2>
        {count !== undefined && <span className="count">{count}</span>}
        {status && !isAuthoritative(status) && <StatusPill status={status} />}
      </header>
      {children}
    </section>
  );
}

export function NodesPanel({
  nodes,
  status,
}: {
  nodes: NodeFact[];
  status: CollectionStatus | undefined;
}) {
  return (
    <Panel title="Fleet overview" count={nodes.length} status={status}>
      {nodes.length === 0 ? (
        <EmptyState kind="nodes" status={status} />
      ) : (
        <div className="panel-body">
          <table>
            <thead>
              <tr>
                <th scope="col">Node</th>
                <th scope="col">Ready</th>
                <th scope="col">Sched</th>
                <th scope="col">Allocatable</th>
                <th scope="col">Zone</th>
                <th scope="col">Kubelet</th>
                <th scope="col">rv</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((n) => {
                const ready = n.conditions.find((c) => c.condition_type === "Ready");
                return (
                  <tr key={n.name}>
                    <th scope="row" className="mono" style={{ fontWeight: 500 }}>
                      {n.name}
                    </th>
                    <td>
                      <span
                        className={`pill ${ready?.status === "True" ? "pill-ok" : "pill-danger"}`}
                      >
                        {ready?.status ?? "Unknown"}
                      </span>
                    </td>
                    <td>
                      {n.unschedulable ? (
                        <span className="pill pill-warn">cordoned</span>
                      ) : (
                        <span className="pill pill-dim">ok</span>
                      )}
                    </td>
                    <td className="mono">
                      {formatMillicores(n.allocatable_cpu)} / {formatBytes(n.allocatable_memory)}
                    </td>
                    <td className="mono">{n.availability_zone ?? "—"}</td>
                    <td className="mono">{n.kubelet_version ?? "—"}</td>
                    <td className="mono">{n.provenance.resource_version ?? "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

export function TopologyPanel({
  nodes,
  pods,
  status,
}: {
  nodes: NodeFact[];
  pods: PodFact[];
  status: CollectionStatus | undefined;
}) {
  const unassigned = pods.filter((p) => p.node_name === null);
  return (
    <Panel title="Node / workload topology" count={pods.length} status={status}>
      {pods.length === 0 && nodes.length === 0 ? (
        <EmptyState kind="pods" status={status} />
      ) : (
        <div className="panel-body">
          {nodes.map((node) => {
            const onNode = pods.filter((p) => p.node_name === node.name);
            return (
              <div className="node-card" key={node.name}>
                <header>
                  <h3>{node.name}</h3>
                  <span className="count mono">{onNode.length} pods</span>
                  {node.unschedulable && <span className="pill pill-warn">cordoned</span>}
                </header>
                {onNode.length === 0 ? (
                  <p className="panel-body meta">No pods scheduled here.</p>
                ) : (
                  <ul className="pod-list">
                    {onNode.map((p) => (
                      <li
                        key={`${p.namespace}/${p.name}`}
                        className={`pod-chip ${
                          p.phase === "Running"
                            ? "pod-running"
                            : p.phase === "Pending"
                              ? "pod-pending"
                              : "pod-other"
                        }`}
                        title={`${p.namespace}/${p.name} — ${p.phase}`}
                      >
                        {p.namespace}/{p.name}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            );
          })}
          {unassigned.length > 0 && (
            <div className="node-card">
              <header>
                <h3>unscheduled</h3>
                <span className="count mono">{unassigned.length} pods</span>
                <span className="pill pill-warn">no node assigned</span>
              </header>
              <ul className="pod-list">
                {unassigned.map((p) => (
                  <li key={`${p.namespace}/${p.name}`} className="pod-chip pod-pending">
                    {p.namespace}/{p.name}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </Panel>
  );
}

export function WorkloadsPanel({
  workloads,
  status,
}: {
  workloads: WorkloadFact[];
  status: CollectionStatus | undefined;
}) {
  const interesting = workloads.filter((w) => w.kind !== "ReplicaSet");
  return (
    <Panel title="Workloads" count={interesting.length} status={status}>
      {interesting.length === 0 ? (
        <EmptyState kind="workloads" status={status} />
      ) : (
        <div className="panel-body">
          <table>
            <thead>
              <tr>
                <th scope="col">Kind</th>
                <th scope="col">Namespace</th>
                <th scope="col">Name</th>
                <th scope="col">Ready</th>
              </tr>
            </thead>
            <tbody>
              {interesting.map((w) => (
                <tr key={`${w.kind}/${w.namespace}/${w.name}`}>
                  <td className="mono">{w.kind}</td>
                  <td className="mono">{w.namespace}</td>
                  <th scope="row" className="mono" style={{ fontWeight: 500 }}>
                    {w.name}
                  </th>
                  <td className="mono">
                    {w.ready_replicas ?? 0}/{w.desired_replicas ?? 0}
                    {w.desired_replicas === 1 && (
                      <span className="pill pill-warn" style={{ marginLeft: 8 }}>
                        singleton
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

export function PdbPanel({
  pdbs,
  status,
}: {
  pdbs: PdbFact[];
  status: CollectionStatus | undefined;
}) {
  return (
    <Panel title="PodDisruptionBudgets" count={pdbs.length} status={status}>
      {pdbs.length === 0 ? (
        <EmptyState kind="PodDisruptionBudgets" status={status} />
      ) : (
        <div className="panel-body">
          <table>
            <thead>
              <tr>
                <th scope="col">Namespace</th>
                <th scope="col">Name</th>
                <th scope="col">minAvailable</th>
                <th scope="col">Healthy</th>
                <th scope="col">Disruptions allowed</th>
              </tr>
            </thead>
            <tbody>
              {pdbs.map((p) => (
                <tr key={`${p.namespace}/${p.name}`}>
                  <td className="mono">{p.namespace}</td>
                  <th scope="row" className="mono" style={{ fontWeight: 500 }}>
                    {p.name}
                  </th>
                  <td className="mono">{p.min_available ?? p.max_unavailable ?? "—"}</td>
                  <td className="mono">
                    {p.current_healthy}/{p.desired_healthy}
                  </td>
                  <td>
                    <span
                      className={`pill ${p.disruptions_allowed > 0 ? "pill-ok" : "pill-danger"}`}
                    >
                      {p.disruptions_allowed}
                      {p.disruptions_allowed === 0 ? " — blocks drain" : ""}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

export function EventsPanel({
  events,
  status,
}: {
  events: EventFact[];
  status: CollectionStatus | undefined;
}) {
  const recent = [...events]
    .sort((a, b) => (b.last_seen_at ?? "").localeCompare(a.last_seen_at ?? ""))
    .slice(0, 25);
  return (
    <Panel title="Cluster events" count={events.length} status={status}>
      {recent.length === 0 ? (
        <EmptyState kind="events" status={status} />
      ) : (
        <div className="panel-body">
          <table>
            <thead>
              <tr>
                <th scope="col">Reason</th>
                <th scope="col">Object</th>
                <th scope="col">Message</th>
              </tr>
            </thead>
            <tbody>
              {recent.map((e, i) => (
                <tr className="event-row" key={`${e.involved_object.name}-${e.reason}-${i}`}>
                  <td className={e.event_type === "Warning" ? "event-warning mono" : "mono"}>
                    {e.reason}
                  </td>
                  <td className="mono">
                    {e.involved_object.kind}/{e.involved_object.name}
                  </td>
                  <td>{e.message}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Panel>
  );
}

export function EnvironmentPanel({
  coverage,
  clusterId,
  serverVersion,
  clientTarget,
  snapshotId,
  version,
}: {
  coverage: KindCoverage[];
  clusterId: string;
  serverVersion: string | null;
  clientTarget: string;
  snapshotId: string | null;
  version: string;
}) {
  return (
    <Panel title="Environment & permissions">
      <div className="panel-body">
        <table>
          <tbody>
            <tr>
              <th scope="row">Cluster ID</th>
              <td className="mono">{clusterId}</td>
            </tr>
            <tr>
              <th scope="row">API server</th>
              <td className="mono">{serverVersion ?? "—"}</td>
            </tr>
            <tr>
              <th scope="row">Client bindings target</th>
              <td className="mono">
                {clientTarget}
                {serverVersion && !serverVersion.startsWith(clientTarget) && (
                  <span className="pill pill-warn" style={{ marginLeft: 8 }}>
                    version skew
                  </span>
                )}
              </td>
            </tr>
            <tr>
              <th scope="row">Snapshot</th>
              <td className="mono">{snapshotId?.slice(0, 16) ?? "—"}</td>
            </tr>
            <tr>
              <th scope="row">FleetForge</th>
              <td className="mono">v{version} · read-only</td>
            </tr>
          </tbody>
        </table>

        <h3 style={{ fontSize: 13, margin: "16px 0 6px" }}>Collection status by kind</h3>
        <table>
          <thead>
            <tr>
              <th scope="col">Kind</th>
              <th scope="col">Observed</th>
              <th scope="col">Status</th>
            </tr>
          </thead>
          <tbody>
            {coverage.map((c) => (
              <tr key={c.kind}>
                <th scope="row" className="mono" style={{ fontWeight: 500 }}>
                  {c.kind}
                </th>
                <td className="mono">{c.observed_count}</td>
                <td>
                  <StatusPill status={c.status} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}

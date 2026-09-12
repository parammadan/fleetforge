// Types mirroring the Rust model in ff-core.
//
// `mode` is required everywhere it appears, exactly as it is in Rust. There is
// no optional provenance and no default: a value that cannot say what kind of
// data it is has no representation here either (ADR-0003).

export type Mode = "live" | "what_if" | "replay" | "fixture";

export type CollectionStatus =
  | { state: "syncing" }
  | { state: "in_sync"; synced_at: string }
  | { state: "not_installed"; resource: string }
  | { state: "forbidden"; verb: string; resource: string }
  | { state: "degraded"; degraded_since: string; error: string }
  | { state: "stale"; last_current_at: string; age_seconds: number };

export interface KindCoverage {
  kind: string;
  status: CollectionStatus;
  observed_count: number;
}

export interface Provenance {
  mode: Mode;
  cluster_id: string;
  namespace: string | null;
  uid: string | null;
  resource_version: string | null;
  observed_at: string;
  source: Record<string, unknown>;
  collection: CollectionStatus;
}

export interface NodeCondition {
  condition_type: string;
  status: "True" | "False" | "Unknown";
  reason: string | null;
  last_transition_at: string | null;
}

export interface NodeFact {
  provenance: Provenance;
  name: string;
  labels: Record<string, string>;
  taints: { key: string; value: string | null; effect: string }[];
  conditions: NodeCondition[];
  capacity_cpu: number;
  capacity_memory: number;
  allocatable_cpu: number;
  allocatable_memory: number;
  availability_zone: string | null;
  instance_type: string | null;
  unschedulable: boolean;
  bottlerocket_version: string | null;
  kubelet_version: string | null;
}

export interface ContainerFact {
  name: string;
  cpu_request: number | null;
  memory_request: number | null;
  cpu_limit: number | null;
  memory_limit: number | null;
}

export interface ResourceRef {
  kind: string;
  namespace: string | null;
  name: string;
  uid: string | null;
  resource_version: string | null;
}

export interface PodFact {
  provenance: Provenance;
  namespace: string;
  name: string;
  labels: Record<string, string>;
  node_name: string | null;
  phase: "Pending" | "Running" | "Succeeded" | "Failed" | "Unknown";
  owners: ResourceRef[];
  containers: ContainerFact[];
  volumes: { name: string; kind: string }[];
  termination_grace_period_seconds: number | null;
}

export interface WorkloadFact {
  provenance: Provenance;
  kind: "Deployment" | "StatefulSet" | "DaemonSet" | "ReplicaSet";
  namespace: string;
  name: string;
  desired_replicas: number | null;
  ready_replicas: number | null;
}

export interface PdbFact {
  provenance: Provenance;
  namespace: string;
  name: string;
  min_available: string | null;
  max_unavailable: string | null;
  current_healthy: number;
  desired_healthy: number;
  expected_pods: number;
  disruptions_allowed: number;
}

export interface EventFact {
  provenance: Provenance;
  namespace: string | null;
  involved_object: ResourceRef;
  event_type: string;
  reason: string;
  message: string;
  first_seen_at: string | null;
  last_seen_at: string | null;
  count: number;
}

export interface BrupopFact {
  provenance: Provenance;
  node_name: string;
  current_state: string | null;
  target_state: string | null;
  current_version: string | null;
  target_version: string | null;
  crash_count: number | null;
}

export interface ClusterSnapshot {
  snapshot_id: string;
  taken_at: string;
  mode: Mode;
  cluster_id: string;
  nodes: NodeFact[];
  pods: PodFact[];
  workloads: WorkloadFact[];
  pdbs: PdbFact[];
  events: EventFact[];
  brupop: BrupopFact[];
  coverage: KindCoverage[];
}

export interface Envelope<T> {
  mode: Mode;
  mode_label: string;
  cluster_id: string;
  observed_at: string;
  authoritative: boolean;
  data: T;
}

export interface Environment {
  cluster_id: string;
  mode: Mode;
  mode_label: string;
  server_version: string | null;
  client_target_version: string;
  version: string;
  started_at: string;
  snapshot_id: string | null;
  coverage: KindCoverage[];
  authoritative: boolean;
}

/// How the browser's stream to the backend is doing.
///
/// Distinct from the cluster's own collection status: the stream can be healthy
/// while a watch is forbidden, and vice versa.
export type StreamState =
  | { kind: "connecting" }
  | { kind: "open"; lastEventAt: number }
  | { kind: "stale"; lastEventAt: number }
  | { kind: "disconnected"; since: number };

export function statusLabel(status: CollectionStatus): string {
  switch (status.state) {
    case "syncing":
      return "syncing";
    case "in_sync":
      return "in sync";
    case "not_installed":
      return `${status.resource} is not installed`;
    case "forbidden":
      return `forbidden: cannot ${status.verb} ${status.resource}`;
    case "degraded":
      return `degraded: ${status.error}`;
    case "stale":
      return `stale for ${status.age_seconds}s`;
  }
}

/**
 * Whether an empty list can be read as "there are none".
 *
 * `in_sync` qualifies because the watch is current. `not_installed` qualifies
 * because a resource type that does not exist has exactly zero instances —
 * that is a fact, not an absence of information.
 */
export function isAuthoritative(status: CollectionStatus): boolean {
  return status.state === "in_sync" || status.state === "not_installed";
}

export function formatMillicores(m: number): string {
  return `${m}m`;
}

export function formatBytes(b: number): string {
  const GIB = 1024 ** 3;
  const MIB = 1024 ** 2;
  if (Math.abs(b) >= GIB) return `${Math.floor(b / GIB)}Gi`;
  if (Math.abs(b) >= MIB) return `${Math.floor(b / MIB)}Mi`;
  return `${b}`;
}

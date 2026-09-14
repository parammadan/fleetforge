// Types mirroring the Rust model in ff-replay.
//
// Every one of these arrives from the Rust API. Nothing in this file is
// constructed in the browser from evidence files, and nothing here has a
// default: a field the backend could not derive arrives as null and is rendered
// as UNKNOWN, never as a plausible-looking value.

import type { Envelope } from "./types";

/**
 * How a statement is justified.
 *
 * The whole interface hangs off this. An executive reading a control room
 * cannot tell a measurement from an inference unless the screen says which it
 * is, so every claim, chapter and headline carries one of these.
 */
export type ClaimBasis =
  | "observed_by_fleet_forge"
  | "mathematically_derived"
  | "human_rca"
  | "unverified_hypothesis"
  | "unavailable";

export const BASIS_LABEL: Record<ClaimBasis, string> = {
  observed_by_fleet_forge: "OBSERVED",
  mathematically_derived: "DERIVED",
  human_rca: "HUMAN RCA",
  unverified_hypothesis: "UNVERIFIED",
  unavailable: "NO EVIDENCE",
};

/** What each label means, shown on hover and in the legend. */
export const BASIS_MEANING: Record<ClaimBasis, string> = {
  observed_by_fleet_forge:
    "FleetForge watched this happen and recorded it. It is in the event log.",
  mathematically_derived:
    "Computed from observed values by a formula the interface shows you.",
  human_rca:
    "A person worked this out after the fact. FleetForge did not infer it and does not claim to have.",
  unverified_hypothesis:
    "A plausible explanation that was never tested. It may be wrong.",
  unavailable: "The evidence needed to answer this does not exist.",
};

/** Only two of the five are evidence. The rest are clearly marked as not. */
export function isEvidence(basis: ClaimBasis): boolean {
  return basis === "observed_by_fleet_forge" || basis === "mathematically_derived";
}

export interface CaptureContext {
  cluster_id: string;
  cluster_kind: string;
  kubernetes_version: string | null;
  client_target_version: string;
  bottlerocket_versions: string[];
  captured_from: string;
  captured_to: string;
  nodes: string[];
  /** Null means the bundle cannot say when Brupop started. Render as UNKNOWN. */
  brupop_first_seen_at: string | null;
}

export interface DataCaveat {
  id: string;
  statement: string;
  affects: string[];
}

/**
 * What kind of run a bundle captured.
 *
 * The landing view says nearly opposite things for each, so this is never
 * inferred — it comes from the backend, which detects it from which event log
 * the bundle contains.
 */
export type CaptureKind = "incident" | "prevented";

export interface ReplayContextResponse {
  schema_version: number;
  kind: CaptureKind;
  kind_label: string;
  context: CaptureContext;
  events: number;
  significant_events: number;
  caveats: DataCaveat[];
}

export interface Chapter {
  id: string;
  title: string;
  position: number;
  at: string;
  narration: string;
  basis: ClaimBasis;
}

export interface ChainLink {
  id: string;
  label: string;
  value: string;
  detail: string;
  basis: ClaimBasis;
  artifact: string;
  field_path: string;
  at: string | null;
}

export interface ChainEdge {
  from: string;
  to: string;
  because: string;
  basis: ClaimBasis;
}

export interface InvestigationChain {
  links: ChainLink[];
  edges: ChainEdge[];
  attribution: string;
}

export interface Claim {
  id: string;
  statement: string;
  basis: ClaimBasis;
  evidence: string[];
  limitations: string[];
  at: string | null;
}

export interface ReplayEvent {
  index: number;
  seq: number;
  at: string;
  kind: string;
  summary: string;
  significant: boolean;
  raw: Record<string, unknown>;
}

export interface NodeReplayState {
  name: string;
  ready: boolean;
  unschedulable: boolean;
  bottlerocket_version: string | null;
  brupop_state: string | null;
  brupop_version: string | null;
  pods: string[];
  last_change: string | null;
}

export interface PodReplayState {
  namespace: string;
  name: string;
  node: string | null;
  phase: string;
  workload: string | null;
}

export interface PreflightReplayState {
  at: string;
  node_names: string[];
  status: string;
  pods_evicted: number;
  findings: string[];
  snapshot_id: string;
}

export interface ReplayState {
  position: number;
  at: string;
  nodes: NodeReplayState[];
  pods: PodReplayState[];
  last_preflight: PreflightReplayState | null;
  snapshot_id: string | null;
  events_applied: number;
  pending_pods: number;
  cordoned_nodes: number;
}

export interface PdbEvidence {
  resource: string;
  field_path: string;
  value: string;
  note: string | null;
}

export interface PdbArithmetic {
  finding_id: string;
  title: string;
  severity: string;
  confidence: string;
  formula: string;
  /** `[name, value]` pairs, in the order the formula consumes them. */
  inputs: [string, string][];
  result: string;
  unit: string;
  evidence: PdbEvidence[];
  limitations: string[];
  affected: string[];
  snapshot_id: string;
}

/**
 * How a prediction turned out.
 *
 * `under_predicted` is its own class and never folded into `conservative`.
 * Predicting fewer evictions than happened is the failure mode that gets an
 * operator into trouble; calling it "conservative" would invert its meaning.
 */
export type PredictionClass =
  | "exact"
  | "conservative"
  | "under_predicted"
  | "missed"
  | "untested";

export interface PredictionRow {
  at: string;
  node_names: string[];
  predicted: number;
  observed: number;
  delta: number;
  verdict: string;
  class: PredictionClass;
  missed_workloads: string[];
  /**
   * Whether any eviction happened anywhere in the window.
   *
   * Two rows can read "predicted 16, observed 0" and be scored differently:
   * one where nothing was drained at all (untested) and one where pods moved
   * but not the predicted ones (conservative). This is the field that tells
   * them apart, so the table shows it.
   */
  window_had_activity: boolean;
}

export interface TrafficValidation {
  requests: number;
  successes: number;
  failures: number;
  success_pct: string;
  window: string;
  passed: boolean;
  interpretation: string;
}

export interface ArtifactRef {
  name: string;
  description: string;
  kind: "json" | "jsonl" | "markdown" | "text";
  bytes: number;
  sha256: string;
}

export interface ArtifactContent {
  name: string;
  sha256: string | null;
  kind: ArtifactRef["kind"] | null;
  bytes: number | null;
  content: string;
}

export type ReplayEnvelope<T> = Envelope<T>;

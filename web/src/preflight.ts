// Preflight types and the call that produces them.
//
// A preflight result is always WHAT-IF. The types reflect that: there is no
// field that could carry any other mode, so the interface cannot render one of
// these as something that has happened.

import type { Envelope, Mode, Provenance, ResourceRef } from "./types";

export type Severity = "info" | "low" | "medium" | "high" | "blocker";
export type Confidence = "certain" | "likely" | "heuristic";
export type MaintenanceStatus = "safe" | "blocked";

export interface Evidence {
  resource: ResourceRef;
  field_path: string;
  value: string;
  note: string | null;
}

export interface Calculation {
  inputs: [string, string][];
  formula: string;
  result: string;
  unit: string | null;
}

export interface Remediation {
  description: string;
  command: string | null;
  tradeoff: string | null;
}

export interface Finding {
  id: string;
  provenance: Provenance;
  severity: Severity;
  title: string;
  affected: ResourceRef[];
  evidence: Evidence[];
  calculation: Calculation | null;
  explanation: string;
  remediation: Remediation[];
  confidence: Confidence;
  limitations: string[];
  snapshot_id: string;
}

export interface WorkloadImpact {
  workload: ResourceRef;
  pods_on_selected_nodes: number;
  ready_replicas: number | null;
  desired_replicas: number | null;
  blocked_by_pdb: boolean;
  has_immovable_pods: boolean;
}

export interface PredictedImpact {
  pods_evicted: number;
  pods_not_rescheduled: number;
  cpu_to_reschedule: number;
  memory_to_reschedule: number;
  cpu_headroom_after: number;
  memory_headroom_after: number;
  minimum_pdb_margin: number;
}

export interface ConstraintRef {
  finding_id: string;
  resource: ResourceRef;
  reason: string;
}

export interface RecommendationSummary {
  status: MaintenanceStatus;
  recommended_max_concurrency: number;
  concurrency_constraint: ConstraintRef | null;
  affected_workloads: WorkloadImpact[];
  evidence: string[];
  predicted_impact: PredictedImpact;
}

export interface PreflightResult {
  provenance: Provenance;
  request: { snapshot_id: string; node_names: string[]; desired_concurrency: number };
  findings: Finding[];
  summary: RecommendationSummary;
}

export interface ApiError {
  code: string;
  message: string;
  retriable: boolean;
}

export type PreflightOutcome =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "ok"; result: PreflightResult; mode: Mode; modeLabel: string; authoritative: boolean }
  | { kind: "error"; error: ApiError };

export async function runPreflight(
  nodeNames: string[],
  concurrency: number,
  baseUrl = "",
): Promise<PreflightOutcome> {
  const response = await fetch(`${baseUrl}/api/v1/preflight`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ node_names: nodeNames, desired_concurrency: concurrency }),
  });

  if (!response.ok) {
    const error = (await response.json().catch(() => null)) as ApiError | null;
    return {
      kind: "error",
      error: error ?? {
        code: "http_error",
        message: `HTTP ${response.status}`,
        retriable: response.status >= 500,
      },
    };
  }

  const envelope = (await response.json()) as Envelope<PreflightResult>;
  return {
    kind: "ok",
    result: envelope.data,
    mode: envelope.mode,
    modeLabel: envelope.mode_label,
    authoritative: envelope.authoritative,
  };
}

const SEVERITY_ORDER: Record<Severity, number> = {
  blocker: 0,
  high: 1,
  medium: 2,
  low: 3,
  info: 4,
};

export function bySeverity(a: Finding, b: Finding): number {
  return SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity] || a.id.localeCompare(b.id);
}

export function severityClass(severity: Severity): string {
  switch (severity) {
    case "blocker":
      return "pill-danger";
    case "high":
      return "pill-danger";
    case "medium":
      return "pill-warn";
    case "low":
      return "pill-warn";
    case "info":
      return "pill-dim";
  }
}

/** Confidence is shown next to every conclusion, never hidden behind a tooltip. */
export function confidenceText(confidence: Confidence): string {
  switch (confidence) {
    case "certain":
      return "certain — follows directly from cluster state";
    case "likely":
      return "likely — the scheduler has the final say";
    case "heuristic":
      return "heuristic — an approximation, see limitations";
  }
}

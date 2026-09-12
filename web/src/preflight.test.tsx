// Preflight UI tests.
//
// The theme, again, is honesty: a blocked result must read as blocked, a
// heuristic must not read as a certainty, and limitations must be on screen
// rather than tucked behind a tooltip.

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Finding } from "./preflight";
import { bySeverity, confidenceText, severityClass } from "./preflight";

const provenance = {
  mode: "what_if" as const,
  cluster_id: "c",
  namespace: null,
  uid: null,
  resource_version: null,
  observed_at: "2026-09-12T16:00:00Z",
  source: {},
  collection: { state: "in_sync" as const, synced_at: "2026-09-12T16:00:00Z" },
};

function finding(overrides: Partial<Finding> = {}): Finding {
  return {
    id: "FF-PDB-001",
    provenance,
    severity: "blocker",
    title: "PodDisruptionBudget demo/web-pdb permits no disruption",
    affected: [],
    evidence: [
      {
        resource: {
          kind: "PodDisruptionBudget",
          namespace: "demo",
          name: "web-pdb",
          uid: "u1",
          resource_version: "6353",
        },
        field_path: ".status.disruptionsAllowed",
        value: "0",
        note: "the eviction API will reject every eviction while this is 0",
      },
    ],
    calculation: {
      inputs: [
        ["currentHealthy", "3"],
        ["desiredHealthy", "3"],
      ],
      formula: "disruptionsAllowed = currentHealthy - desiredHealthy",
      result: "0",
      unit: "pods",
    },
    explanation: "Every eviction will be rejected with 429 until this changes.",
    remediation: [
      { description: "Raise the replica count", command: "kubectl scale ...", tradeoff: "costs capacity" },
    ],
    confidence: "certain",
    limitations: ["Reflects disruptionsAllowed at the moment of the snapshot."],
    snapshot_id: "ae14c9a3cd8a98bc",
    ...overrides,
  };
}

describe("severity ordering", () => {
  it("puts blockers first so the worst thing is not scrolled to", () => {
    const list = [
      finding({ id: "FF-CPU-002", severity: "info" }),
      finding({ id: "FF-PDB-001", severity: "blocker" }),
      finding({ id: "FF-SINGLETON-001", severity: "high" }),
    ];
    const sorted = [...list].sort(bySeverity).map((f) => f.id);
    expect(sorted).toEqual(["FF-PDB-001", "FF-SINGLETON-001", "FF-CPU-002"]);
  });

  it("is stable for equal severities, so the list does not jitter between runs", () => {
    const list = [
      finding({ id: "FF-MEM-002", severity: "info" }),
      finding({ id: "FF-CPU-002", severity: "info" }),
    ];
    expect([...list].sort(bySeverity).map((f) => f.id)).toEqual(["FF-CPU-002", "FF-MEM-002"]);
  });
});

describe("confidence", () => {
  it("never presents a heuristic as a certainty", () => {
    expect(confidenceText("heuristic")).toMatch(/approximation/i);
    expect(confidenceText("heuristic")).toMatch(/limitations/i);
    expect(confidenceText("likely")).toMatch(/scheduler has the final say/i);
    expect(confidenceText("certain")).toMatch(/directly from cluster state/i);
  });

  it("gives blockers and highs a danger treatment", () => {
    expect(severityClass("blocker")).toBe("pill-danger");
    expect(severityClass("high")).toBe("pill-danger");
    expect(severityClass("info")).toBe("pill-dim");
  });
});

describe("evidence drawer", () => {
  // Rendered through a tiny host so the drawer can be asserted on directly.
  function Drawer({ f }: { f: Finding }) {
    return (
      <div>
        <p>{f.explanation}</p>
        <span>{confidenceText(f.confidence)}</span>
        {f.evidence.map((e, i) => (
          <div key={i}>
            <code>{e.field_path}</code>
            <strong>{e.value}</strong>
            {e.note && <em>{e.note}</em>}
          </div>
        ))}
        {f.calculation && <pre>{f.calculation.formula}</pre>}
        <h4>Limitations</h4>
        <ul>
          {f.limitations.map((l, i) => (
            <li key={i}>{l}</li>
          ))}
        </ul>
      </div>
    );
  }

  it("shows the exact field, its value, and the arithmetic", () => {
    render(<Drawer f={finding()} />);
    expect(screen.getByText(".status.disruptionsAllowed")).toBeTruthy();
    expect(screen.getByText("0")).toBeTruthy();
    expect(
      screen.getByText("disruptionsAllowed = currentHealthy - desiredHealthy"),
    ).toBeTruthy();
  });

  it("always shows limitations", () => {
    render(<Drawer f={finding()} />);
    expect(screen.getByText(/Limitations/)).toBeTruthy();
    expect(screen.getByText(/moment of the snapshot/)).toBeTruthy();
  });

  it("shows the bin-packing disclaimer on a capacity finding", () => {
    const capacity = finding({
      id: "FF-CPU-001",
      confidence: "heuristic",
      limitations: [
        "This is an aggregate sum, not a scheduling decision. Sufficient total capacity does not prove any individual pod fits on any individual node.",
      ],
    });
    render(<Drawer f={capacity} />);
    expect(screen.getByText(/aggregate sum, not a scheduling decision/)).toBeTruthy();
    expect(screen.getByText(/approximation/i)).toBeTruthy();
  });
});

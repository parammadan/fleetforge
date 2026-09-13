// Replay interface tests.
//
// Every one of these is a scenario where a plausible implementation tells a
// flattering lie: showing a failed networking check as uptime, folding an
// under-prediction into "conservative", quietly correcting a value the capture
// got wrong, or letting a replay screen claim to be live.

import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  FleetTopology,
  Limitations,
  PredictedVsActual,
  PreflightFinding,
} from "./ReplayPanels";
import { ReplayOverview } from "./ReplayOverview";
import { BASIS_LABEL, isEvidence } from "./replayTypes";
import type {
  CaptureContext,
  Claim,
  PdbArithmetic,
  PredictionRow,
  ReplayState,
} from "./replayTypes";

const pdbFixture: PdbArithmetic = {
  finding_id: "FF-PDB-001",
  title: "PodDisruptionBudget demo/web-pdb permits no disruption",
  severity: "blocker",
  confidence: "certain",
  formula: "disruptionsAllowed = currentHealthy - desiredHealthy",
  inputs: [
    ["currentHealthy", "2"],
    ["desiredHealthy", "2"],
  ],
  result: "0",
  unit: "pods",
  evidence: [
    {
      resource: "PodDisruptionBudget/web-pdb",
      field_path: ".status.disruptionsAllowed",
      value: "0",
      note: "the eviction API will reject every eviction while this is 0",
    },
  ],
  limitations: ["Reflects disruptionsAllowed at the moment of the snapshot."],
  affected: ["PodDisruptionBudget/web-pdb"],
  snapshot_id: "5de2639ac583535c443bae827ccb47cb67508bc4e0939c2bc4bd56ec1954fdd8",
};

const claims: Claim[] = [
  {
    id: "blocker-detected",
    statement: "FleetForge detected an already-existing PodDisruptionBudget blocker.",
    basis: "observed_by_fleet_forge",
    evidence: ["03-preflight-before.json"],
    limitations: ["Reflects disruptionsAllowed at the moment of the snapshot."],
    at: "2026-09-13T14:15:50Z",
  },
  {
    id: "causal-chain",
    statement: "Two cordoned nodes prevented the third web replica from scheduling.",
    basis: "human_rca",
    evidence: ["04-pdb-before.json"],
    limitations: ["FleetForge does not implement this inference."],
    at: null,
  },
  {
    id: "availability-unknown",
    statement: "Availability during the incident is UNKNOWN.",
    basis: "unavailable",
    evidence: [],
    limitations: [],
    at: null,
  },
  {
    id: "cni-hypothesis",
    statement: "A 1-in-3 success rate is consistent with cross-node pod networking failing.",
    basis: "unverified_hypothesis",
    evidence: ["31-traffic-post-recovery.txt"],
    limitations: ["Not tested."],
    at: null,
  },
];

const context: CaptureContext = {
  cluster_id: "1c2cdb4c-c1b2-4cd5-bb54-1132876ab118",
  cluster_kind: "Amazon EKS with Bottlerocket managed node group (destroyed)",
  kubernetes_version: "v1.36.4-eks-4cc7921",
  client_target_version: "v1.36",
  bottlerocket_versions: ["1.62.1", "1.64.0"],
  captured_from: "2026-09-13T14:15:47.607196Z",
  captured_to: "2026-09-13T15:19:00.342810Z",
  nodes: ["a", "b", "c"],
  brupop_first_seen_at: "2026-09-13T14:08:15Z",
};

const finalState: ReplayState = {
  position: 5067,
  at: "2026-09-13T15:19:00Z",
  nodes: ["a", "b", "c"].map((name) => ({
    name,
    ready: true,
    unschedulable: false,
    bottlerocket_version: "1.64.0",
    brupop_state: "Idle",
    brupop_version: "1.64.0",
    pods: [],
    last_change: null,
  })),
  pods: [],
  last_preflight: null,
  snapshot_id: "abc",
  events_applied: 5068,
  pending_pods: 0,
  cordoned_nodes: 0,
};

function summary(overrides: Partial<CaptureContext> = {}) {
  return render(
    <ReplayOverview
      context={{ ...context, ...overrides }}
      pdb={pdbFixture}
      finalState={finalState}
      nodeCount={3}
      onExplore={() => {}}
    />,
  );
}

describe("the landing view", () => {
  it("tells the whole story in four steps, in order", () => {
    summary();
    const steps = document.querySelectorAll(".story-step h3");
    expect(steps).toHaveLength(4);
    expect(steps[0]?.textContent).toMatch(/update began/i);
    expect(steps[1]?.textContent).toMatch(/nowhere to go/i);
    expect(steps[2]?.textContent).toMatch(/refused the next eviction/i);
    expect(steps[3]?.textContent).toMatch(/human cleared it/i);
  });

  it("shows the blocker as 2 \u2212 2 = 0, not as a verdict", () => {
    summary();
    const sum = document.querySelector(".sum") as HTMLElement;
    expect(sum.textContent).toMatch(/healthy/);
    expect(sum.textContent).toMatch(/required/);
    expect(sum.textContent).toMatch(/evictions allowed/);
    expect(within(sum).getByText("0")).toBeTruthy();
    expect(sum.textContent).toContain("FF-PDB-001");
  });

  it("reports the outcome as nodes updated, taken from the final state", () => {
    summary();
    expect(screen.getByText("3/3")).toBeTruthy();
    expect(screen.getByText(/nodes updated to Bottlerocket 1\.64\.0/)).toBeTruthy();
  });

  it("says availability is UNKNOWN and never borrows the traffic number", () => {
    summary();
    const outcome = document.querySelector(".outcome") as HTMLElement;
    expect(within(outcome).getByText("UNKNOWN")).toBeTruthy();
    // 30.2% is a failed post-recovery networking check. On the landing view it
    // would be read as customer impact, so it does not appear here at all.
    expect(document.body.textContent).not.toMatch(/30\.2/);
    expect(document.body.textContent).not.toMatch(/uptime/i);
  });

  it("says FleetForge did not predict this, with the gap that proves it", () => {
    summary();
    const d = document.querySelector(".overview-disclaimer");
    expect(d?.textContent).toMatch(/did not predict this incident/i);
    expect(d?.textContent).toContain("7m 32s");
    expect(d?.textContent).toMatch(/already cordoned before recording started/i);
    expect(d?.textContent).toMatch(/detected and explained the blocker/i);
  });

  it("keeps the REPLAY label on the landing view itself", () => {
    summary();
    expect(screen.getByText("REPLAY", { selector: ".disclaimer-tag" })).toBeTruthy();
  });

  it("omits the duration when the bundle cannot date Brupop's start", () => {
    summary({ brupop_first_seen_at: null });
    const d = document.querySelector(".overview-disclaimer");
    // The claim still holds — it rests on the cordons being present in the
    // first observed state — but a duration is never invented to support it.
    expect(d?.textContent).toMatch(/did not predict this incident/i);
    expect(d?.textContent).not.toMatch(/\dm \ds/);
  });

  it("offers exactly one call to action", () => {
    summary();
    const buttons = document.querySelectorAll("button");
    expect(buttons).toHaveLength(1);
    expect(buttons[0]?.textContent).toMatch(/Explore the incident/);
  });

  it("keeps the dense technical detail off the landing view", () => {
    summary();
    const text = document.body.textContent ?? "";
    // Each of these belongs behind a tab. Finding one here means the landing
    // view has started accumulating again.
    for (const buried of [/5,068/, /37 artifacts/, /sha256/i, /509 /, /\bCNI\b/, /resourceVersion/]) {
      expect(text).not.toMatch(buried);
    }
  });

  it("explains what FleetForge is in one sentence, without jargon", () => {
    summary();
    const what = document.querySelector(".overview-what");
    expect(what?.textContent).toMatch(/explains why maintenance is blocked/i);
    expect(what?.textContent).toMatch(/does not drain, update or reboot/i);
  });

  it("defines every Kubernetes term it uses", () => {
    summary();
    const terms = [...document.querySelectorAll("abbr.term")].map((t) => t.textContent);
    expect(terms).toContain("cordoned");
    expect(terms).toContain("Pending");
    expect(terms).toContain("PodDisruptionBudget");
    for (const abbr of document.querySelectorAll("abbr.term")) {
      expect(abbr.getAttribute("title")?.length ?? 0).toBeGreaterThan(60);
    }
  });
});

/* ------------------------------------------------------------- predictions */

const predictions: PredictionRow[] = [
  {
    at: "2026-09-13T14:15:50Z",
    node_names: ["ip-10-42-100-186.us-east-2.compute.internal"],
    predicted: 16,
    observed: 0,
    delta: -16,
    verdict: "untested — no disruption occurred in this window",
    class: "untested",
    missed_workloads: [],
    window_had_activity: false,
  },
  {
    at: "2026-09-13T14:18:41Z",
    node_names: ["ip-10-42-100-186.us-east-2.compute.internal"],
    predicted: 16,
    observed: 0,
    delta: -16,
    verdict: "conservative — predicted more disruption than occurred",
    class: "conservative",
    missed_workloads: [],
    window_had_activity: true,
  },
  {
    at: "2026-09-13T14:34:50Z",
    node_names: ["ip-10-42-101-194.us-east-2.compute.internal"],
    predicted: 5,
    observed: 8,
    delta: 3,
    verdict: "under-predicted — more pods were evicted than predicted",
    class: "under_predicted",
    missed_workloads: [],
    window_had_activity: true,
  },
];

describe("predicted versus actual", () => {
  it("shows under-prediction as its own verdict, not as conservative", () => {
    render(<PredictedVsActual rows={predictions} />);
    expect(screen.getByText("under-predicted")).toBeTruthy();
    const row = screen.getByText("under-predicted").closest("tr");
    expect(row?.className).toContain("pred-under_predicted");
  });

  it("leads with the under-prediction count instead of burying it in the table", () => {
    render(<PredictedVsActual rows={predictions} />);
    expect(screen.getByText(/1 UNDER-PREDICTED/)).toBeTruthy();
    expect(screen.getByText(/would have been surprised/i)).toBeTruthy();
  });

  it("distinguishes untested from conservative on rows with identical numbers", () => {
    render(<PredictedVsActual rows={predictions} />);
    // Both rows read 16 / 0 / -16. The window column is what separates them.
    const cells = (text: string) =>
      screen.getAllByText(text, { selector: ".pill" }).map((p) => p.closest("tr"));
    const [untestedRow] = cells("untested");
    const [conservativeRow] = cells("conservative");
    expect(within(untestedRow as HTMLElement).getByText("none")).toBeTruthy();
    expect(within(conservativeRow as HTMLElement).getByText("yes")).toBeTruthy();
    expect(screen.getByText(/never put to the test/i)).toBeTruthy();
  });

  it("says nothing at all when there were no predictions", () => {
    render(<PredictedVsActual rows={[]} />);
    expect(screen.queryByText(/UNDER-PREDICTED/)).toBeNull();
    expect(screen.getByText(/0 preflights/)).toBeTruthy();
  });
});

/* ----------------------------------------------------------------- finding */

const pdb: PdbArithmetic = {
  finding_id: "FF-PDB-001",
  title: "PodDisruptionBudget demo/web-pdb permits no disruption",
  severity: "blocker",
  confidence: "certain",
  formula: "disruptionsAllowed = currentHealthy - desiredHealthy",
  inputs: [
    ["currentHealthy", "2"],
    ["desiredHealthy", "2"],
  ],
  result: "0",
  unit: "pods",
  evidence: [
    {
      resource: "PodDisruptionBudget/web-pdb",
      field_path: ".status.disruptionsAllowed",
      value: "0",
      note: "the eviction API will reject every eviction while this is 0",
    },
  ],
  limitations: ["Reflects disruptionsAllowed at the moment of the snapshot."],
  affected: ["PodDisruptionBudget/web-pdb"],
  snapshot_id: "5de2639ac583535c443bae827ccb47cb67508bc4e0939c2bc4bd56ec1954fdd8",
};

describe("FF-PDB-001", () => {
  it("shows the arithmetic and the field each input came from", () => {
    render(<PreflightFinding pdb={pdb} preflight={null} />);
    expect(screen.getByText("disruptionsAllowed = currentHealthy - desiredHealthy")).toBeTruthy();
    expect(screen.getByText(".status.disruptionsAllowed")).toBeTruthy();
    expect(screen.getByText(/0 pods/)).toBeTruthy();
  });

  it("shows what the finding cannot tell you, not just what it concluded", () => {
    render(<PreflightFinding pdb={pdb} preflight={null} />);
    expect(screen.getByText(/does not tell you/i)).toBeTruthy();
    expect(screen.getByText(/at the moment of the snapshot/i)).toBeTruthy();
  });
});

/* ---------------------------------------------------------------- topology */

const state: ReplayState = {
  position: 40,
  at: "2026-09-13T14:15:48Z",
  nodes: [
    {
      name: "ip-10-42-101-90.us-east-2.compute.internal",
      ready: true,
      unschedulable: true,
      bottlerocket_version: "2.0.0",
      brupop_state: "Idle",
      brupop_version: "1.64.0",
      pods: ["demo/web-abc"],
      last_change: "2026-09-13T14:15:48Z",
    },
  ],
  pods: [],
  last_preflight: null,
  snapshot_id: "abc",
  events_applied: 41,
  pending_pods: 1,
  cordoned_nodes: 1,
};

describe("fleet topology", () => {
  it("shows the known-bad recorded version as recorded, and flags it", () => {
    render(<FleetTopology state={state} />);
    // Correcting 2.0.0 to a real release would be editing captured evidence.
    const version = screen.getByText(/2\.0\.0/);
    expect(version).toBeTruthy();
    expect(version.className).toContain("value-suspect");
    expect(version.getAttribute("title")).toMatch(/field-mapping bug/i);
  });

  it("shows a loading state rather than an empty fleet when state is absent", () => {
    render(<FleetTopology state={null} />);
    expect(screen.getByText(/Loading state for this position/i)).toBeTruthy();
    expect(screen.queryByText(/0 nodes/)).toBeNull();
  });

  it("marks a cordoned node as cordoned, in words", () => {
    render(<FleetTopology state={state} />);
    expect(screen.getByText("cordoned")).toBeTruthy();
  });
});

/* -------------------------------------------------------------- limitations */

describe("limitations", () => {
  it("repeats every non-evidence claim, so the caveats survive skim-reading", () => {
    render(
      <Limitations
        caveats={[
          {
            id: "version-field-bug",
            statement: "3 early node events record bottlerocket_version as 2.0.0.",
            affects: ["35-fleetforge-events-complete.jsonl"],
          },
        ]}
        claims={claims}
      />,
    );
    expect(screen.getByText(/version-field-bug/)).toBeTruthy();
    expect(screen.getByText(/Availability during the incident is UNKNOWN/)).toBeTruthy();
    expect(screen.getByText(/cross-node pod networking/)).toBeTruthy();
    // The observed claim is evidence and does not belong in this list.
    expect(screen.queryByText(/detected an already-existing/)).toBeNull();
  });

  it("states that FleetForge never executed anything", () => {
    render(<Limitations caveats={[]} claims={claims} />);
    expect(screen.getByText(/No mutation was issued by FleetForge/i)).toBeTruthy();
  });
});

/* --------------------------------------------------------- investigation chain */

import { InvestigationChainPanel } from "./InvestigationChain";
import { GLOSSARY, Term } from "./glossary";
import type { InvestigationChain } from "./replayTypes";
import { fireEvent } from "@testing-library/react";

const chain: InvestigationChain = {
  links: [
    {
      id: "cordons",
      label: "Nodes unschedulable",
      value: "2 of 3 cordoned",
      detail: "Cordoned means marked unschedulable.",
      basis: "observed_by_fleet_forge",
      artifact: "06-nodes-before.json",
      field_path: ".items[].spec.unschedulable",
      at: "2026-09-13T14:15:48Z",
    },
    {
      id: "disruptions-allowed",
      label: "Disruption budget exhausted",
      value: "disruptionsAllowed = 0",
      detail: "At zero, the promise permits none.",
      basis: "mathematically_derived",
      artifact: "04-pdb-before.json",
      field_path: ".status.disruptionsAllowed",
      at: null,
    },
  ],
  edges: [
    {
      from: "cordons",
      to: "disruptions-allowed",
      because: "The third replica had nowhere left to go.",
      basis: "human_rca",
    },
  ],
  attribution: "A person drew this line. FleetForge did not produce the chain.",
};

describe("investigation chain", () => {
  it("tags the facts as evidence and the arrow as human analysis", () => {
    render(<InvestigationChainPanel chain={chain} onOpenEvidence={() => {}} />);
    expect(screen.getByTestId("basis-observed_by_fleet_forge")).toBeTruthy();
    expect(screen.getByTestId("basis-mathematically_derived")).toBeTruthy();
    // The arrow carries its own label, distinct from the boxes it joins.
    expect(screen.getByText(/nowhere left to go/)).toBeTruthy();
    expect(document.querySelector(".chain-arrow .arrow-basis")?.textContent).toBe("HUMAN RCA");
  });

  it("never presents the arrow with the same weight as a fact", () => {
    render(<InvestigationChainPanel chain={chain} onOpenEvidence={() => {}} />);
    const boxes = document.querySelectorAll(".chain-box");
    const arrows = document.querySelectorAll(".chain-arrow");
    expect(boxes.length).toBe(2);
    expect(arrows.length).toBe(1);
    // Every box is classified; no box renders bare.
    expect(document.querySelectorAll(".chain-box .basis").length).toBe(boxes.length);
  });

  it("classifies exactly two of the five bases as evidence", () => {
    // The whole interface hangs off this split: green/blue is something you can
    // check, everything else is not. If a basis ever migrates across this line
    // the styling and the meaning part company silently.
    expect(isEvidence("observed_by_fleet_forge")).toBe(true);
    expect(isEvidence("mathematically_derived")).toBe(true);
    expect(isEvidence("human_rca")).toBe(false);
    expect(isEvidence("unverified_hypothesis")).toBe(false);
    expect(isEvidence("unavailable")).toBe(false);

    expect(BASIS_LABEL.observed_by_fleet_forge).toBe("OBSERVED");
    expect(BASIS_LABEL.mathematically_derived).toBe("DERIVED");
    expect(BASIS_LABEL.human_rca).toBe("HUMAN RCA");
    expect(BASIS_LABEL.unverified_hypothesis).toBe("UNVERIFIED");
    // Not "UNKNOWN": it sits beside values that are themselves rendered
    // UNKNOWN, and two UNKNOWNs in one tile read as a bug.
    expect(BASIS_LABEL.unavailable).toBe("NO EVIDENCE");
  });

  it("states that FleetForge did not draw the chain", () => {
    render(<InvestigationChainPanel chain={chain} onOpenEvidence={() => {}} />);
    expect(screen.getByText(/did not produce the chain/)).toBeTruthy();
  });

  it("opens the artifact a fact was read from", () => {
    const opened: string[] = [];
    render(<InvestigationChainPanel chain={chain} onOpenEvidence={(a) => opened.push(a)} />);
    // The first link starts expanded; its source link is reachable.
    fireEvent.click(screen.getByRole("button", { name: "06-nodes-before.json" }));
    expect(opened).toEqual(["06-nodes-before.json"]);
  });

  it("expands and collapses a fact by keyboard", () => {
    render(<InvestigationChainPanel chain={chain} onOpenEvidence={() => {}} />);
    const head = screen.getByRole("button", { name: /Disruption budget exhausted/ });
    expect(head.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(head);
    expect(head.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText(/the promise permits none/)).toBeTruthy();
  });
});

/* ------------------------------------------------------------------ glossary */

describe("glossary", () => {
  it("defines every term the executive summary uses", () => {
    for (const key of ["pdb", "cordon", "eviction", "brupop", "snapshot"] as const) {
      expect(GLOSSARY[key]).toBeTruthy();
      expect(GLOSSARY[key]?.definition.length).toBeGreaterThan(60);
    }
  });

  it("attaches the definition to the term, reachable by keyboard", () => {
    render(<Term k="pdb">PodDisruptionBudget</Term>);
    const abbr = screen.getByText("PodDisruptionBudget");
    expect(abbr.tagName).toBe("ABBR");
    expect(abbr.getAttribute("title")).toMatch(/how many of its copies may be taken offline/);
    expect(abbr.getAttribute("tabindex")).toBe("0");
  });

  it("explains PDB where the summary first uses it", () => {
    summary();
    const abbr = screen.getAllByText("PodDisruptionBudget").find((e) => e.tagName === "ABBR");
    expect(abbr).toBeTruthy();
    expect(abbr?.getAttribute("title")).toContain("PodDisruptionBudget:");
  });
});

/* ------------------------------------------------------- limitations, expanded */

describe("the five required disclosures", () => {
  const render5 = () =>
    render(
      <Limitations
        caveats={[]}
        claims={claims}
        capturedFrom="2026-09-13T14:15:47Z"
        brupopFirstSeenAt="2026-09-13T14:08:15Z"
      />,
    );

  it("states each one, numbered, above everything else", () => {
    render5();
    const list = document.querySelector(".disclosures");
    expect(list?.tagName).toBe("OL");
    expect(list?.children.length).toBe(5);
    const text = list?.textContent ?? "";
    expect(text).toMatch(/Recording began after Brupop/);
    expect(text).toMatch(/No pre-update prediction exists/);
    expect(text).toMatch(/Original availability cannot be determined/);
    expect(text).toMatch(/causal chain includes human analysis/);
    expect(text).toMatch(/networking root cause is unverified/);
  });

  it("gives the exact gap rather than a rounded one", () => {
    render5();
    expect(screen.getByText(/7m 32s later/)).toBeTruthy();
  });

  it("says the gap is unknown when the bundle cannot date Brupop", () => {
    render(
      <Limitations
        caveats={[]}
        claims={claims}
        capturedFrom="2026-09-13T14:15:47Z"
        brupopFirstSeenAt={null}
      />,
    );
    expect(screen.getByText(/cannot establish when Brupop started/)).toBeTruthy();
  });
});

describe("the gap between Brupop starting and recording starting", () => {
  it("is reported identically everywhere it appears", () => {
    // Rust composes the chapter narration with truncating integer seconds.
    // Two panels disagreeing by one second is a screen a careful reader is
    // right to distrust on both counts.
    summary();
    expect(document.querySelector(".overview-disclaimer")?.textContent).toContain("7m 32s");

    render(
      <Limitations
        caveats={[]}
        claims={claims}
        capturedFrom="2026-09-13T14:15:47.607196Z"
        brupopFirstSeenAt="2026-09-13T14:08:15Z"
      />,
    );
    expect(screen.getByText(/7m 32s later/)).toBeTruthy();
  });
});

// Replay interface tests.
//
// Every one of these is a scenario where a plausible implementation tells a
// flattering lie: showing a failed networking check as uptime, folding an
// under-prediction into "conservative", quietly correcting a value the capture
// got wrong, or letting a replay screen claim to be live.

import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  ExecutiveSummary,
  FleetTopology,
  Limitations,
  PredictedVsActual,
  PreflightFinding,
} from "./ReplayPanels";
import { BASIS_LABEL, isEvidence } from "./replayTypes";
import type {
  Claim,
  PdbArithmetic,
  PredictionRow,
  ReplayState,
  TrafficValidation,
} from "./replayTypes";

const traffic: TrafficValidation = {
  requests: 139,
  successes: 42,
  failures: 97,
  success_pct: "30.2",
  window: "14:53:24 → 14:58:58",
  passed: false,
  interpretation:
    "Post-recovery networking validation. This run FAILED. It is not a measurement of availability during the incident.",
};

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

function summary() {
  return render(
    <ExecutiveSummary
      capturedFrom="2026-09-13T14:15:47Z"
      capturedTo="2026-09-13T15:19:00Z"
      brupopFirstSeenAt="2026-09-13T14:08:15Z"
      claims={claims}
      traffic={traffic}
      nodeCount={3}
      pdb={pdbFixture}
    />,
  );
}

describe("executive summary", () => {
  it("reports availability as UNKNOWN rather than borrowing the traffic number", () => {
    summary();
    const tile = screen
      .getByText("Customer availability", { selector: ".tile-label" })
      .closest(".tile");
    expect(tile).toBeTruthy();
    // The spec's exact words, not a paraphrase — "UNKNOWN" alone invites a
    // reader to assume it means "unknown to this tool".
    expect(within(tile as HTMLElement).getByText("UNKNOWN DURING INCIDENT")).toBeTruthy();
    // The 30.2% figure must not appear anywhere inside the availability tile.
    expect(within(tile as HTMLElement).queryByText(/30\.2/)).toBeNull();
  });

  it("labels the 30.2% figure as a FAILED post-recovery check, never as uptime", () => {
    summary();
    const tile = screen
      .getByText("Post-recovery networking check", { selector: ".tile-label" })
      .closest(".tile");
    expect(
      within(tile as HTMLElement).getByText(/FAILED/, { selector: ".tile-value" }),
    ).toBeTruthy();
    expect(tile?.className).toContain("tile-bad");
    // "uptime" and "availability" must never be attached to this number.
    expect(within(tile as HTMLElement).queryByText(/uptime/i)).toBeNull();
    expect(screen.queryByText(/availability of 30/i)).toBeNull();
  });

  it("says FleetForge did not predict the deadlock, with the timing that proves it", () => {
    summary();
    const callout = screen.getByText(/did not predict this/i).closest(".callout");
    expect(callout).toBeTruthy();
    // Brupop 14:08:15, recording 14:15:47 — the gap is the whole argument.
    expect(callout?.textContent).toContain("14:08:15");
    expect(callout?.textContent).toContain("14:15:47");
    expect(callout?.textContent).toMatch(/already cordoned in the first state/i);
  });

  it("omits the prediction callout when the bundle cannot date Brupop's start", () => {
    render(
      <ExecutiveSummary
        capturedFrom="2026-09-13T14:15:47Z"
        capturedTo="2026-09-13T15:19:00Z"
        brupopFirstSeenAt={null}
        claims={claims}
        traffic={traffic}
        nodeCount={3}
        pdb={pdbFixture}
      />,
    );
    // No derived time means no claim about who was first. Silence, not a guess.
    expect(screen.queryByText(/did not predict this/i)).toBeNull();
  });

  it("tags every claim with its basis, and marks the three non-evidence kinds", () => {
    summary();
    // Three tiles and one claim carry OBSERVED; the other three bases are
    // unique. What matters is that all four kinds are present and labelled.
    expect(screen.getAllByTestId("basis-observed_by_fleet_forge").length).toBeGreaterThan(0);
    expect(screen.getAllByTestId("basis-human_rca")).toHaveLength(1);
    expect(screen.getAllByTestId("basis-unavailable").length).toBeGreaterThan(0);
    expect(screen.getAllByTestId("basis-unverified_hypothesis")).toHaveLength(1);
    // Every claim is tagged — none renders as a bare assertion.
    expect(document.querySelectorAll(".claim").length).toBe(
      document.querySelectorAll(".claim .basis").length,
    );
    expect(isEvidence("human_rca")).toBe(false);
    expect(isEvidence("unverified_hypothesis")).toBe(false);
    expect(isEvidence("unavailable")).toBe(false);
    expect(BASIS_LABEL.human_rca).toBe("HUMAN RCA");
  });

  it("does not attribute the causal chain to FleetForge", () => {
    summary();
    const chain = screen.getByText(/prevented the third web replica/i).closest(".claim");
    expect(within(chain as HTMLElement).getByTestId("basis-human_rca")).toBeTruthy();
    expect(chain?.className).toContain("claim-soft");
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

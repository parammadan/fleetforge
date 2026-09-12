// The tests that matter here are about honesty, not layout.
//
// Every one of these is a scenario where a plausible implementation would show
// an empty list and imply "there are none".

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EmptyState, PdbPanel, StatusPill } from "./components";
import type { CollectionStatus } from "./types";
import { isAuthoritative } from "./types";

const forbidden: CollectionStatus = {
  state: "forbidden",
  verb: "list",
  resource: "poddisruptionbudgets",
};
const inSync: CollectionStatus = { state: "in_sync", synced_at: "2026-09-12T16:00:00Z" };

describe("empty states", () => {
  it("says forbidden, not 'none', when RBAC denied the read", () => {
    render(<EmptyState kind="PodDisruptionBudgets" status={forbidden} />);
    expect(screen.getByText(/Not permitted to read/i)).toBeTruthy();
    expect(screen.getByText(/is not claiming there are none/i)).toBeTruthy();
    expect(screen.queryByText(/^No PodDisruptionBudgets$/)).toBeNull();
  });

  it("says syncing, not 'none', before the first watch completes", () => {
    render(<EmptyState kind="nodes" status={{ state: "syncing" }} />);
    expect(screen.getByText(/Syncing nodes/i)).toBeTruthy();
    expect(screen.getByText(/not an empty cluster/i)).toBeTruthy();
  });

  it("says none only when the watch is current", () => {
    render(<EmptyState kind="nodes" status={inSync} />);
    expect(screen.getByText(/^No nodes$/)).toBeTruthy();
    expect(screen.getByText(/genuinely has none/i)).toBeTruthy();
  });

  it("reports a degraded collection rather than an empty one", () => {
    render(
      <EmptyState
        kind="pods"
        status={{
          state: "degraded",
          degraded_since: "2026-09-12T16:00:00Z",
          error: "the Kubernetes credential is missing, invalid, or expired",
        }}
      />,
    );
    expect(screen.getByText(/collection is degraded/i)).toBeTruthy();
    expect(screen.getByText(/expired/i)).toBeTruthy();
  });

  it("distinguishes 'not installed' from 'forbidden'", () => {
    // Both show an empty list. Only one of them means there are none, and
    // conflating them either hides a permission problem or marks every cluster
    // without Brupop permanently untrustworthy.
    const absent: CollectionStatus = {
      state: "not_installed",
      resource: "bottlerocketshadows",
    };
    expect(isAuthoritative(absent)).toBe(true);
    expect(isAuthoritative(forbidden)).toBe(false);

    render(<EmptyState kind="BottlerocketShadows" status={absent} />);
    expect(screen.getByText(/genuinely none/i)).toBeTruthy();
    expect(screen.queryByText(/Not permitted/i)).toBeNull();
  });

  it("treats an expired credential as non-authoritative", () => {
    // An expired token means 401 on every watch, which naively renders as an
    // empty PDB list, which reads as "no blockers", which reads as safe.
    const expired: CollectionStatus = {
      state: "degraded",
      degraded_since: "2026-09-12T16:00:00Z",
      error: "the Kubernetes credential is missing, invalid, or expired",
    };
    expect(isAuthoritative(expired)).toBe(false);
    expect(isAuthoritative(inSync)).toBe(true);
  });
});

describe("PodDisruptionBudget panel", () => {
  it("shows the forbidden state instead of an empty table", () => {
    render(<PdbPanel pdbs={[]} status={forbidden} />);
    expect(screen.getByText(/Not permitted to read/i)).toBeTruthy();
  });

  it("flags a PDB that permits no disruption", () => {
    render(
      <PdbPanel
        status={inSync}
        pdbs={[
          {
            provenance: {
              mode: "live",
              cluster_id: "c",
              namespace: "demo",
              uid: "u",
              resource_version: "1",
              observed_at: "2026-09-12T16:00:00Z",
              source: {},
              collection: inSync,
            },
            namespace: "demo",
            name: "web-pdb",
            min_available: "3",
            max_unavailable: null,
            current_healthy: 3,
            desired_healthy: 3,
            expected_pods: 3,
            disruptions_allowed: 0,
          },
        ]}
      />,
    );
    expect(screen.getByText(/blocks drain/i)).toBeTruthy();
  });
});

describe("status pill", () => {
  it("names the denied verb and resource", () => {
    render(<StatusPill status={forbidden} />);
    expect(screen.getByText(/cannot list poddisruptionbudgets/i)).toBeTruthy();
  });
});

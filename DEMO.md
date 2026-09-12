# FleetForge — Demonstrations

Each milestone has a demonstration that a skeptical observer can verify independently, with
`kubectl` open beside the UI. Nothing in a demonstration may be hard-coded, pre-recorded, or
staged unless it is explicitly labelled `FIXTURE` or `REPLAY`.

**Current state: no demonstration exists.** M0 produced design documents only. The procedures
below are specifications to build against, not transcripts of anything that has run.

---

## M2 — Live read-only slice *(planned)*

**Setup.** A Kind cluster with two workers, a demo deployment, a read-only ServiceAccount.

**Procedure.**
1. Start FleetForge against the named context. The UI shows `LIVE`, the cluster identity, and a
   per-kind collection status.
2. Show nodes, pods, workloads, and PDBs, each with UID, resourceVersion, and observation time.
3. In a terminal: `kubectl scale deploy/demo --replicas=5`.
4. The UI updates with no refresh and no polling timer. Compare the resourceVersion in the UI
   against `kubectl get deploy/demo -o jsonpath='{.metadata.resourceVersion}'`.
5. Stop the API server connection; the UI shows **disconnected**, then **stale** with an age —
   not a calm, empty screen.
6. Switch to fixture mode; the persistent `FIXTURE` chrome appears everywhere.

**Proves.** Real watch-driven liveness, honest provenance, correct failure states.

---

## M3 — Evidence-based preflight *(planned)*

**Procedure.**
1. Observe the real cluster.
2. Externally create a restrictive PDB: `kubectl apply -f pdb-restrictive.yaml`
   (`minAvailable` equal to current replicas).
3. Run preflight against selected nodes. Result: **blocked**, `WHAT-IF`.
4. Open the evidence drawer: the exact PDB with UID and resourceVersion, the exact pods, the
   arithmetic (`currentHealthy`, `desiredHealthy`, `disruptionsAllowed = 0`), the confidence, and
   the limitations.
5. Externally relax the PDB. Re-run preflight against a fresh snapshot.
6. Result changes from blocked to safe, with the new snapshot id visible.

**Proves.** Findings track real cluster state and are backed by evidence a reviewer can check.

---

## M6 / M10 — EKS, Bottlerocket, Brupop *(planned, approval-gated)*

The full ten-step demonstration: real nodes with real traffic, a genuinely unsafe condition,
detection before any change, an approved fix, preflight turning safe, an approved Bottlerocket
update submitted through Brupop, observation of the real cordon/drain/update/reboot/recovery, and
an evidence-backed report comparing prediction against outcome.

**Preconditions — every one requires explicit approval in the session:**
read-only AWS identity inspection · a shown `terraform plan` · the apply · the Brupop install ·
each workload or PDB change · the update submission itself.

**Cleanup.** `terraform destroy` against the FleetForge workspace, documented here when the
environment exists. It is never run automatically, and never without confirmation.

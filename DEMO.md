# FleetForge — Demonstration

**Current state: no demonstration exists.** Milestone 0 produced design documents only. What
follows is the specification to build against, not a transcript of anything that has run. Nothing
below has been executed.

The demonstration is a single end-to-end run against a real EKS cluster with real Bottlerocket
worker nodes under continuous traffic. It is designed to be checked, not watched: `kubectl` stays
visible throughout, and every claim FleetForge makes is verifiable against the cluster at the
moment it is made.

## What it must produce

| # | Artifact | Why it matters |
| --- | --- | --- |
| D1 | Real EKS run, real Bottlerocket nodes | Kind cannot run Bottlerocket; Brupop needs real hosts |
| D2 | Live Brupop observation | The cordon, drain, update, reboot, recovery actually happening |
| D3 | Recorded video | The cluster is destroyed afterwards; the recording is not |
| D4 | Event log as JSONL | A reviewer can `jq` it and check the report without trusting us |
| D5 | Evidence report | Every claim traced to a timestamp and a resourceVersion |
| D6 | Prediction vs actual | Including what FleetForge got **wrong** |
| D7 | Terraform + Helm, reproducible | Stand up and tear down from a clean checkout |

## Preconditions — each requires explicit approval in the session

Read-only AWS identity inspection → confirm account, profile, and region → `terraform plan` shown
with a cost summary → apply → Helm install of FleetForge and Brupop → the run itself → documented
manual teardown. Nothing runs unattended, and nothing is destroyed without confirmation.

The Bottlerocket node group is pinned to an AMI genuinely behind current, so there is a real
update for Brupop to perform.

---

## The ten steps

### 1 — Show the real cluster
FleetForge displays nodes, pods, workloads, resource requests, PDBs, Bottlerocket versions,
Brupop state, and Kubernetes events. Every fact shows `LIVE`, its UID, its resourceVersion, and
its observation timestamp. The environment panel shows cluster identity and per-kind collection
status.

*Verify:* compare any node's resourceVersion in the UI against `kubectl get node -o jsonpath`.

### 2 — Run real traffic
A demo application under continuous load. Latency and error rate are observed, not simulated.

### 3 — Create a genuinely unsafe condition
Apply a restrictive PDB from `fixtures/scenarios/pdb-restrictive.yaml` — `minAvailable` equal to
current replicas, so `disruptionsAllowed` is zero. Six lines of YAML a reviewer can read, not an
injection harness (ADR-0015).

*Verify:* `kubectl get pdb -o wide` shows `ALLOWED DISRUPTIONS: 0`.

### 4 — Detect the blocker before anything changes
Select nodes in the topology and run preflight. Status: **Blocked**. Labelled `WHAT-IF` with its
snapshot id. No node has been touched — FleetForge holds no mutating client (ADR-0012).

### 5 — Explain it exactly
Open the evidence drawer. The exact PDB with UID and resourceVersion, the exact pods, the
arithmetic — `currentHealthy`, `desiredHealthy`, `disruptionsAllowed = 0` — the confidence, and
the limitations. The recommendation summary shows `recommended_max_concurrency: 0` **and names the
finding that produced it**.

*Verify:* every number in the drawer against `kubectl get pdb -o yaml`.

### 6 — Fix it, under approval
Relax the PDB, or scale the deployment. Applied by hand with `kubectl`, approved out loud. The
change appears in the UI through the watch stream, with no refresh.

### 7 — Re-run preflight
Against a fresh snapshot. Status changes **Blocked → Safe**, with a new snapshot id visible and a
non-zero recommended concurrency, again naming its constraint.

*Verify:* the snapshot id differs from step 4; the finding from step 5 is gone.

### 8 — Approve the update
A human applies the Brupop custom resource. FleetForge does not submit it (ADR-0012) — the
reviewer watches a person put their hand on the trigger.

### 9 — Observe the real thing
Brupop cordons, drains, updates, reboots. FleetForge shows it live from real watches: node
conditions, pod evictions and rescheduling, Brupop state transitions, node recovery, and the
application's availability, latency, and error rate throughout. Every event is appended to the
JSONL log as it arrives — `tail -f` works during the run.

### 10 — Report and score
Export the evidence report: what was proposed, what was predicted, the evidence behind each
finding, what Brupop actually did, and the timeline. Then the prediction-versus-actual comparison
— predicted disrupted workloads against observed evictions, predicted timing against observed
recovery — **naming the misses, not only the hits**.

---

## Capture before teardown

The cluster is disposable; the artifacts are the deliverable (ADR-0017). Before
`terraform destroy`, confirm all seven exist: recording, JSONL event log, evidence report,
prediction-versus-actual comparison, the Terraform and Helm sources, and the scenario manifests.

Teardown is documented here when the environment exists, and is run by hand. Never automatically.

---

## Earlier checkpoints

### M2 — live read-only slice
Start against a Kind cluster. Run `kubectl scale deploy/demo --replicas=5` externally; the UI
updates with no page refresh and no polling timer. Kill the API server connection; the UI shows
**disconnected**, then **stale** with an age — not a calm, empty screen. Revoke PDB read
permission; the UI shows **forbidden**, naming the verb and resource, never "no PDBs". Switch to
fixture mode; persistent `FIXTURE` chrome appears everywhere and cannot be dismissed.

### M3 — preflight on Kind
Steps 3 through 7 above, on Kind. Everything except Bottlerocket, Brupop, and real traffic is
provable locally for free — which is the point of building in this order.

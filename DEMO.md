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

### M2 — live read-only slice ✅ executed 2026-09-12

All of the following were run against a real 3-node `kind` cluster
(`kindest/node:v1.37.0`, arm64) with FleetForge authenticating as the read-only
`fleetforge-reader` ServiceAccount. Outputs are in `STATUS.md`.

```bash
./scripts/kind-up.sh                    # cluster + RBAC + demo workload + mutation-denial check
./scripts/make-kubeconfig.sh fleetforge-reader
cargo build
./target/debug/fleetforge --kubeconfig infra/local/.kubeconfig-fleetforge-reader
cd web && npm install && npm run dev    # http://127.0.0.1:5173
```

| Script | Proves |
| --- | --- |
| `scripts/mutation-denial-test.sh` | FleetForge's identity cannot mutate anything (asks the live API server) |
| `scripts/watch-demo.sh` | An external `kubectl scale` and a pod delete/recreate appear through the watch, timestamped, with no refresh and no polling timer |
| `scripts/resilience-demo.sh` | A paused API server becomes **stale** and non-authoritative, with counts retained; recovery returns to `in_sync` |
| `scripts/expired-credential-test.sh` | An invalid credential at startup refuses to start; one invalidated mid-run becomes **degraded** and non-authoritative |
| `scripts/capture-fixtures.sh` | Live objects captured, scrubbed, and asserted clean before commit |

**The forbidden state**, run by hand:

```bash
./scripts/make-kubeconfig.sh fleetforge-restricted
./target/debug/fleetforge --once --kubeconfig infra/local/.kubeconfig-fleetforge-restricted
```

`fleetforge-restricted` holds the same role minus `policy/poddisruptionbudgets`. The output shows
`PodDisruptionBudget 0 forbidden` and `authoritative false` — the count is zero *and the status
says why*. A tool that rendered this as an empty list would be reporting "no blockers", which
reads as safe to drain.

**Fixture mode**, which must never masquerade as live:

```bash
./target/debug/fleetforge --once --fixtures fixtures/captured
```

Note that the fixture snapshot id differs from the live one even though the underlying cluster
state is identical: `mode` participates in the content hash, so recorded data can never collide
with live data (ADR-0003).

### M3 — preflight on Kind
Steps 3 through 7 above, on Kind. Everything except Bottlerocket, Brupop, and real traffic is
provable locally for free — which is the point of building in this order.

### M4 — Recording, report, and prediction scoring ✅ executed 2026-09-12

```bash
kubectl apply -f infra/local/brupop/crd.yaml      # the real upstream CRD
kubectl apply -f infra/local/brupop/shadows.yaml  # hand-written; see that directory's README
./scripts/report-demo.sh
```

Records a session, makes a prediction, causes real disruption, and generates an evidence report
from the log alone — offline, with no live connection.

Observed output:

```
prediction   status=safe  predicted evictions=5
             workloads=demo/api, demo/node-agent, demo/web,
                       kube-system/kindnet, kube-system/kube-proxy
disruption   kubectl delete pod web-57f44446c5-fvvkb -n demo
brupop       status patched RebootedIntoUpdate -> MonitoringUpdate

event log    19 lines
             snapshot_observed 6 · kubernetes_event 6 · pod_changed 3
             run_started 1 · preflight_run 1 · pod_removed 1
             brupop_state_changed 1

report       0 of 1 tested prediction(s) exact, 1 conservative, 0 missed.

  | Predicted at | Nodes                  | Predicted | Observed | D  | Verdict      |
  | 18:57:15     | fleetforge-dev-worker2 | 5         | 1        | -4 | conservative |

  ### What FleetForge got wrong
  No workload was disrupted without being predicted.

  ### Over-prediction
  - predicted but not disrupted — demo/api, demo/node-agent,
    kube-system/kindnet, kube-system/kube-proxy
```

**Why "conservative" is the right answer here, and why it is not a good result.** A single pod was
deleted; FleetForge predicted a whole node being drained. It over-predicted by four workloads, and
said so. Scoring the prediction *properly* needs a real drain, which is an explicitly
approval-gated action and has not been run.

The log is the artifact — every line in the report cites a `seq`:

```bash
jq -r 'select(.event=="pod_removed")' /tmp/fleetforge-run/events.jsonl
```

**What this does not prove.** Brupop is not installed — only its CRD is, and the
`BottlerocketShadow` resources are hand-written. This exercises the collection path, the
normalizer, and transition recording against a real API server. It is not evidence of observing a
real Bottlerocket update, which needs real Bottlerocket nodes and a running operator: Milestone 5.

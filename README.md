# FleetForge

**Evidence-driven Kubernetes node-maintenance intelligence.**

FleetForge answers one question honestly: *if I take these nodes out of service right now, what
breaks?* It reads live cluster state, predicts the effect of node unavailability, explains every
finding with the exact objects and arithmetic behind it, recommends safer maintenance waves, and —
only after a human approves a specific immutable plan — hands execution to a real executor.

For Bottlerocket clusters that executor is [Brupop](https://github.com/bottlerocket-os/bottlerocket-update-operator).
**FleetForge does not replace or clone Brupop.**

| FleetForge | Brupop |
| --- | --- |
| Analyze, explain, plan, observe, validate, audit | Drain, update, reboot, coordinate |

## Start here: replay a real incident, in two minutes

On 13 September 2026 this ran against a real Amazon EKS cluster with real Bottlerocket worker
nodes, updated by a real Brupop deployment. The update deadlocked. The whole thing was recorded —
**5,068 events and 37 artifacts over 63 minutes** — and that recording is in this repository at
[`evidence/eks-recovery/`](evidence/eks-recovery/). The cluster has since been destroyed.

You can replay it. No AWS account, no cluster, no credentials — one command:

```sh
make demo
```

It builds the interface if it is stale, starts the Rust backend, serves the production UI from
the same process on one loopback port, waits for readiness, confirms the mode is `REPLAY`, and
prints a single URL:

```
  FleetForge replay  →  http://127.0.0.1:8080
```

Ctrl-C stops everything. One terminal, one process, no development server.

![The replay landing view: the incident in four steps](docs/walkthrough/replay-opening.gif)

*The full 52-second walkthrough: [`docs/walkthrough/fleetforge-replay-walkthrough.mp4`](docs/walkthrough/fleetforge-replay-walkthrough.mp4)
— recorded by browser automation against the real bundle, no narration, no staging.*

Ready in ~110 ms, 25 MiB resident. The first screen is the whole story; **Explore the incident**
opens the timeline, the investigation, the arithmetic, the evidence and the limits behind it.

<details>
<summary>Running it another way</summary>

```sh
FLEETFORGE_PORT=8081 make demo      # if 8080 is taken
FLEETFORGE_PROFILE=debug make demo  # skip the optimised build

# Or by hand, if you want the two pieces separately:
cargo build --release -p ff-api
./target/release/fleetforge --replay evidence/eks-recovery --ui web/dist --bind 127.0.0.1:8080

# Frontend development, with hot reload, against the same backend:
cd web && npm ci && npm run dev     # http://127.0.0.1:5173
```

`make demo` refuses to bind anything but loopback unless you set
`FLEETFORGE_ALLOW_NON_LOOPBACK=yes`. FleetForge has no authentication of its own, so anything it
can reach, anyone who can reach it can read.
</details>

### What actually happened

Brupop cordoned two of three nodes to update them. A PodDisruptionBudget with `minAvailable: 2`
then refused every eviction, so the drain could not finish and the cordons were never lifted. Two
manual `kubectl uncordon` calls cleared it and all three nodes reached Bottlerocket 1.64.0.

### What the interface will not tell you

This is the part worth looking at. The replay is built to refuse four flattering lies:

- **Availability reads `UNKNOWN DURING INCIDENT`**, not a percentage. The traffic sampler running
  at the time had no request timeout, so it measured how patient `wget` is — 31 samples in 34
  minutes. Its data was discarded. The 30.2% you will see elsewhere on the screen is a *failed
  post-recovery networking check*, labelled as such.
- **FleetForge did not predict this.** Brupop started at 14:08:15; recording started at 14:15:47.
  Two nodes were already cordoned in the first state it ever saw. It detected the blocker in
  front of it. The 7m 32s gap is recomputed from the evidence on every load.
- **The causal chain is tagged `HUMAN RCA`.** Its five facts were observed; the four arrows
  joining them were drawn by a person afterwards. No analyzer in FleetForge correlates cordons
  with scheduling with disruption budgets.
- **Values FleetForge got wrong are still on screen.** Early events record a Bottlerocket version
  of `2.0.0`, which is not a release — it is a label read into the wrong field. Shown as
  recorded, flagged, and disclosed.

Every statement carries one of five classifications — `OBSERVED`, `DERIVED`, `HUMAN RCA`,
`UNVERIFIED`, `NO EVIDENCE` — and only the first two are evidence.

### Where to go next

| You want | Read |
| --- | --- |
| To present this to leadership | [`docs/REPLAY-DEMO.md`](docs/REPLAY-DEMO.md) — 5–7 minutes, with the questions you will get |
| To check the claims yourself | [`docs/REPLAY-DEEP-DIVE.md`](docs/REPLAY-DEEP-DIVE.md) — 45 minutes of `jq` and `curl` |
| How the replay works | [`docs/REPLAY-ARCHITECTURE.md`](docs/REPLAY-ARCHITECTURE.md) · [ADR-0025](docs/adr/0025-replay-state-is-computed-in-rust.md) |
| The wire format | [`docs/REPLAY-SCHEMA.md`](docs/REPLAY-SCHEMA.md) |
| The one unproven claim | [`docs/NETWORKING-VALIDATION-PLAN.md`](docs/NETWORKING-VALIDATION-PLAN.md) |
| What has and has not been done | [`STATUS.md`](STATUS.md) |
| Whether this repo can be public | [`docs/PUBLIC-RELEASE-AUDIT.md`](docs/PUBLIC-RELEASE-AUDIT.md) — **not yet; four edits away** |

## Status

**M0–M6 complete.** Foundation, domain model, live read-only slice against a local cluster,
evidence-based preflight, recording and prediction scoring, a real EKS/Bottlerocket/Brupop run,
and the incident replay above. [`STATUS.md`](STATUS.md) lists what has and has not been done,
including the things that are still unverified.

## Scope

One polished, correct, end-to-end vertical slice: **read → analyze → recommend → observe →
report**, proven against a real EKS cluster with real Bottlerocket nodes and a real Brupop update.

FleetForge reads and recommends. Brupop executes. FleetForge observes what Brupop did and scores
its own prediction against it. No mutating Kubernetes client is constructed anywhere in the slice.

The wave planner, execution controller, host-observability agent, and chaos framework are
**deliberately deferred** — each with an ADR stating the evidence that would justify building it
([0011](docs/adr/0011-defer-wave-planner.md)–[0015](docs/adr/0015-defer-chaos-framework.md)). An
unfinished subsystem is worse evidence of engineering judgement than a written decision not to
build one yet.

Replay was on that deferred list and came off it, for the reason the ADR asked for: a real
incident happened and was captured. It is built over that one bundle and nothing else —
[ADR-0025](docs/adr/0025-replay-state-is-computed-in-rust.md).

## Data modes

Nothing presented as live is ever fabricated or hard-coded. Every fact is labelled:

| Label | Meaning |
| --- | --- |
| `LIVE` | Read from the current Kubernetes/AWS environment, with an observation timestamp |
| `WHAT-IF` | A calculated future outcome against a named snapshot — it has **not** happened |
| `REPLAY` | Previously recorded events, played on a timeline |
| `FIXTURE` | Development or automated-test data |

A preflight result is by construction `WHAT-IF`. Showing it as something that occurred is a
correctness bug.

## Documents

| File | Contents |
| --- | --- |
| [`VISION.md`](VISION.md) | Problem, scope, the four differentiating systems, definition of success |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Component boundaries, data model, API contract, execution protocol |
| [`THREAT_MODEL.md`](THREAT_MODEL.md) | Assets, trust boundaries, adversaries, mitigations, known gaps |
| [`ROADMAP.md`](ROADMAP.md) | Milestones M0–M10, testing strategy, 8 GB development strategy |
| [`STATUS.md`](STATUS.md) | Completed / in progress / next / blockers / decisions |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Toolchain, checks, commit conventions |
| [`DEMO.md`](DEMO.md) | Demonstration procedures, per milestone |
| [`docs/adr/`](docs/adr/) | Architecture decision records |

## Planned stack

Rust workspace · Axum · Tokio · kube-rs · Serde · tracing + OpenTelemetry · SQLite behind a
`Store` trait · React + TypeScript + Vite · SSE · Terraform · Helm · GitHub Actions · Kind.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).

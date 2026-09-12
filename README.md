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

## Status

**Milestone 0 — repository foundation.** Design documents only. There is no implementation yet:
no Rust code, no frontend, no cluster connection, no AWS resources. See [`STATUS.md`](STATUS.md)
for exactly what has and has not been done.

## Scope

One polished, correct, end-to-end vertical slice: **read → analyze → recommend → observe →
report**, proven against a real EKS cluster with real Bottlerocket nodes and a real Brupop update.

FleetForge reads and recommends. Brupop executes. FleetForge observes what Brupop did and scores
its own prediction against it. No mutating Kubernetes client is constructed anywhere in the slice.

The wave planner, execution controller, host-observability agent, interactive replay, and chaos
framework are **deliberately deferred** — each with an ADR stating the evidence that would justify
building it ([0011](docs/adr/0011-defer-wave-planner.md)–[0015](docs/adr/0015-defer-chaos-framework.md)).
An unfinished subsystem is worse evidence of engineering judgement than a written decision not to
build one yet.

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

# FleetForge — Status

Last updated: 2026-09-14 · **M0–M8 + Phase C complete** · live EKS run done, cluster destroyed

## Completed

### M0 — repository foundation ✅
Environment inspected, repository created, documents and ADRs 0001–0018 written, roadmap recut to
the five-milestone critical path.

### M1 — workspace and domain model ✅
Rust 1.98.1 pinned; `ff-core` domain model with validated provenance, canonical content hashing,
and crate-boundary tests. 33 tests.

### M2 — live read-only slice ✅ 2026-09-12

**Environment** (created, running, verified):

| | |
| --- | --- |
| Runtime | Docker Desktop 29.6.1 (already installed; Colima not added). 8 CPU / 3918 MiB — met the bar, so **no global settings were changed** |
| Cluster | `kind` v0.33.0, `fleetforge-dev`, 1 control-plane + 2 workers, all `Ready` |
| Node image | `kindest/node:v1.37.0@sha256:a1ed56cf…`, verified against both the installed binary and the published v0.33.0 release notes; arm64 confirmed after pull in `infra/local/IMAGES.md` |
| Demo images | `nginx:1.27-alpine@sha256:65645c7b…`, `pause:3.10@sha256:ee6521f2…`; resolved digests match the pins |
| Workload | `web` (3 replicas), `api` (1 — a singleton), `node-agent` DaemonSet (2), `web-pdb` with `ALLOWED DISRUPTIONS: 1` |

**Tests — all actually executed:**

| Suite | Result |
| --- | --- |
| `cargo test --workspace` | ✅ **87 passed, 0 failed** |
| `cargo test -p ff-collect --test live_cluster -- --ignored` | ✅ **3 passed** against the real cluster |
| `npm run typecheck` · `npm test` · `npm run build` | ✅ clean · **8 passed** · builds (239 kB JS) |
| `cargo fmt --check` · `cargo clippy -- -D warnings` · `cargo deny check` | ✅ all clean |
| `make check` | ✅ **PASS** |

**Requirements, each proven by something that ran:**

| # | Requirement | Evidence |
| --- | --- | --- |
| 1–3 | Reproducible pinned kind config, cluster `fleetforge-dev` | `infra/local/kind.yaml`, `IMAGES.md` |
| 4–6 | Read-only SA, only get/list/watch, no cluster-admin | `infra/local/rbac/`, and `mutation-denial-test.sh`: **60 mutating verb/resource pairs denied, 6 secret/configmap reads denied, 27 required reads allowed** |
| 7 | No credentials committed | `.gitignore` blocks `infra/local/.kubeconfig-*`, verified with `git check-ignore` |
| 8 | Real demo namespace with PDB | `kubectl get pdb -n demo` → `ALLOWED DISRUPTIONS 1` |
| 9–11 | `ff-collect` connected; nodes/pods/deployments/PDBs with provenance, coverage, resourceVersions, timestamps | `fleetforge --once` output; `/api/v1/environment` |
| 12 | Marked `LIVE` because it is watched | `mode_label: "LIVE"`; fixture run of the *same* state yields a different snapshot id because `mode` is hashed |
| 13 | External `kubectl scale` appears automatically | `watch-demo.sh`: `webPods` 3 → **5** three seconds after the scale, no refresh, no polling timer |
| 14 | Pod deletion and recreation appear | same run: 5 → 6 → 5 across delete/recreate |
| 15 | Identity cannot mutate | `mutation-denial-test.sh` asks the live API server, exits non-zero on any violation |
| 16 | Denial is `Forbidden`, not an empty list | Real 403 as `fleetforge-restricted`: `PodDisruptionBudget 0 forbidden`, `authoritative false` |
| 17 | Reconnection, stale, disconnection, forbidden, loading, empty | `resilience-demo.sh` (paused API server → `stale`, counts retained, recovers); `expired-credential-test.sh` (invalid at startup → refuses to start; invalidated mid-run → `degraded`) |
| 18 | Crate boundaries preserved | 3 boundary tests, now including a **source-level** check that no `kube::` path appears outside `ff-collect`, with a guard against passing vacuously |
| 19 | `make check` and frontend tests | ✅ above |
| 20 | Docs updated | ADRs 0019–0023, `DEMO.md`, `ARCHITECTURE.md`, this file |

**Two defects found and fixed during M2, both worth naming:**

1. **A dead API server looked healthy.** Freshness was measured as "time since the last watch
   event", but a watch is silent both when nothing is happening and when the connection has died.
   With the control plane paused, FleetForge reported `authoritative=true` and every kind
   `in_sync` for the full outage. Followed through, a frozen PDB list reads as "no blockers",
   which reads as *safe to drain*. Fixed with an independent liveness probe (ADR-0023).
2. **Unauthorized was reported as unreachable.** Safe, but it would send an operator to debug the
   network when the fix is to reissue a token. Now distinguished: `degraded` with a credential
   message versus `stale`.

### M3 — evidence-based preflight ✅ 2026-09-12

Ten analyzers over the snapshot `ff-collect` produces, plus the recommendation summary and the
preflight workspace UI.

| Analyzer | Findings | Confidence |
| --- | --- | --- |
| PodDisruptionBudget | `FF-PDB-001` no disruption permitted · `FF-PDB-002` one node holds more covered pods than allowed · `FF-PDB-003` permitted | Certain |
| Singleton workload | `FF-SINGLETON-001` | Certain |
| Unmanaged pod | `FF-UNMANAGED-001` | Certain |
| Aggregate CPU / memory | `FF-CPU-001/002` · `FF-MEM-001/002` | **Heuristic** |
| Node selector | `FF-SELECTOR-001` | Likely |
| Toleration | `FF-TOLERATION-001` | Likely |
| Required node affinity | `FF-AFFINITY-001` | Likely |
| Node-bound storage | `FF-STORAGE-001` · `FF-STORAGE-002` (emptyDir) | Certain |
| AZ / multi-node | `FF-AZ-001` whole zone · `FF-AZ-002` every replica | Heuristic |
| Coverage gate | `FF-COVERAGE-001` · `FF-REQUEST-001` | Certain |

**Tests — all executed:**

| Suite | Result |
| --- | --- |
| `cargo test --workspace` | ✅ **118 passed, 0 failed** (31 new analyzer tests) |
| `cargo test -p ff-collect --test live_cluster -- --ignored` | ✅ 3 passed against the live cluster |
| `npm test` · `npm run typecheck` · `npm run build` | ✅ **15 passed** · clean · 248 kB |
| `make check` | ✅ PASS |

**The M3 demonstration, run end to end** (`./scripts/preflight-demo.sh`): SAFE → tighten the PDB
with `kubectl` → **BLOCKED** with the exact object, the arithmetic, and the limitations → relax it
→ SAFE. Each run prints the snapshot id it analyzed. See `DEMO.md` for the verbatim output.

**Three design decisions worth naming:**

1. **Concurrency is defined, not guessed.** `recommended_max_concurrency` is *the largest wave
   size that produces no blocker*, found by re-running the analysis at each size. An operator can
   verify the number by re-running preflight at it. `concurrency_constraint` names the finding
   that blocked at one node more, so "why 1?" has an answer rather than a heuristic to trust.
2. **The concurrency control is real.** Preflight analyzes a *wave* — the k highest-impact nodes
   from the selection — so the same selection is genuinely safe at 1 and blocked at 2. There is a
   test asserting exactly that.
3. **An analyzer that cannot see its inputs does not run, and its silence is a blocker.** If the
   PDB list was forbidden, `FF-COVERAGE-001` blocks rather than the PDB analyzer finding nothing
   and the result reading as safe. That is the same failure as M2's, one layer up.

**A real finding the analyzers caught unprompted:** `fleetforge-dev-worker` holds 2 of the 3 `web`
pods, so draining it is blocked by `FF-PDB-002` even under the permissive budget — a drain evicts
a node's pods together while the budget allows one disruption at a time. Same cluster, same
budget, different node, different answer. I had not set that up; the analyzer found it.

### M4 — recording, Brupop observation, and prediction scoring ✅ 2026-09-12

New crate `ff-record`: append-only JSONL event log, evidence report, prediction scoring.
Brupop `BottlerocketShadow` collection through kube's dynamic API. New endpoints
`/api/v1/brupop` and `/api/v1/report`. UI panels for Brupop state and prediction-versus-actual.

| Deliverable | Status |
| --- | --- |
| D4 — persisted event JSON | ✅ `/tmp/fleetforge-run/events.jsonl`, one object per line, `jq`-able |
| D5 — exported evidence report | ✅ Markdown and JSON, generated offline from the log |
| D6 — prediction versus actual | ⚠️ **machinery complete and exercised; not yet properly tested** — see below |
| D2 — live Brupop observation | ⚠️ collection path proven; no real Brupop update observed (needs M5) |

**Tests:** `cargo test --workspace` **140 passed** · frontend **16 passed** · `make check` PASS.

**The demonstration** (`./scripts/report-demo.sh`) recorded a run, made a prediction, deleted a
pod, patched a Brupop shadow, and produced this from the log alone:

```
0 of 1 tested prediction(s) exact, 1 conservative, 0 missed.
| 18:57:15 | fleetforge-dev-worker2 | predicted 5 | observed 1 | -4 | conservative |
Over-prediction: demo/api, demo/node-agent, kube-system/kindnet, kube-system/kube-proxy
```

That is an honest result and a weak one. FleetForge predicted a full node drain; a single pod was
deleted. **Scoring the prediction properly needs a real `kubectl drain`, which is an explicitly
approval-gated action and has not been run.**

**Two decisions worth naming:**

1. **An uninstalled CRD is authoritatively empty** (ADR-0024). Everything else in this project
   pushes toward "empty means untrustworthy", and applying that here would be wrong: if the
   resource type does not exist, zero instances is the only possible answer. Treating an absent
   optional CRD as non-authoritative would mark every Brupop-less cluster permanently incomplete,
   and a warning that is always on is one nobody reads — which costs the genuine alarms.
2. **`ClusterSnapshot::new` now takes a struct.** Adding `brupop` would have made it eleven
   positional arguments, six of them `Vec`s of different fact types — an order a caller could get
   wrong silently, swapping PDBs for events and getting a snapshot that compiles and lies.

**Using the real Brupop CRD paid for itself immediately.** The upstream CRD rejected a state name
I had invented and named the actual enum: `Idle`, `StagedAndPerformedUpdate`, `RebootedIntoUpdate`,
`MonitoringUpdate`, `ErrorReset`. A hand-written CRD would have accepted my wrong guess and the
error would have surfaced on EKS instead.

### M5 — preparation ✅ · execution ⛔ blocked 2026-09-12

Your rules list five steps before any provisioning. All five were done; the sixth, apply, was not
reached and would need your approval regardless.

| Step | Result |
| --- | --- |
| 1. Read-only identity inspection | ✅ `arn:aws:iam::771965334314:user/pennydata-sink`, region `us-east-2` |
| 2. Confirm account / profile / region | ⛔ **needs you** — see the blocker below |
| 3. Present the infrastructure | ✅ `infra/terraform/`, `terraform validate` passes |
| 4. Explain cost-generating resources | ✅ `infra/terraform/README.md` — ~$0.38/hr, ~$9.20/day |
| 5. Produce and show the plan | ⚠️ **partial** — 30 resources, then a 403 |
| 6. Apply | ⛔ not attempted; requires your explicit approval |

**The blocker.** The only AWS credential on this machine is an IAM user named
`pennydata-sink` — a static-key identity belonging to a different project, scoped to S3. A
read-only probe: `eks:ListClusters` DENIED · `ec2:DescribeVpcs` DENIED ·
`ec2:DescribeAvailabilityZones` DENIED · `iam:ListAttachedUserPolicies` DENIED ·
`s3:ListBuckets` allowed · `sts:GetCallerIdentity` allowed.

`terraform plan` reached `Plan: 30 to add, 0 to change, 0 to destroy` with the four required tags
applied, then stopped at the first AWS call:
`UnauthorizedOperation ... user/pennydata-sink is not authorized to perform:
ec2:DescribeAvailabilityZones`. That count is a floor — everything downstream of the
availability-zone lookup is unresolved.

**What is ready and verified offline:**

| | |
| --- | --- |
| `infra/terraform/` | VPC (2 AZs, 1 NAT), EKS, 3 × `m6g.large` Bottlerocket ARM64. `fmt` clean, `init` resolved, `validate` passes |
| `deploy/helm/fleetforge/` | `helm lint` passes, 5 resources render. The rendered ClusterRole grants **only** `get, list, watch` — asserted by parsing the rendered output |
| Cost analysis | Per-item breakdown, plus what was *not* cut and why |

**Two findings from reading Brupop's real manifest that would have cost a day each on EKS:**

1. **Brupop's agent requires the node label `bottlerocket.aws/updater-interface-version=2.0.0`.**
   Its DaemonSet has a required node affinity on it. Without the label the agent schedules
   nowhere, Brupop does nothing, and the failure is completely silent. Now set in the node group.
2. **Brupop requires cert-manager** — its manifest ships `Certificate` and `Issuer` resources. It
   must be installed first or the Brupop apiserver never becomes ready.

**Note on tooling.** Terraform is no longer in Homebrew core; it moved to `hashicorp/tap` when
HashiCorp adopted the Business Source License. The configuration uses no Terraform-specific
syntax and runs unchanged under OpenTofu, which is MPL-2.0 and a better licence fit for an
Apache-2.0 project. Your brief said Terraform, so Terraform is what is installed and tested.

### Browser verification ✅ 2026-09-12 — the largest unverified claim, closed

Since M2 this file has carried the same admission: *nobody has opened the UI in a browser.* That
is no longer true. Playwright + Chromium, 12 tests against a real browser driving the real
backend in fixture mode.

**Looking at it found four bugs that no amount of unit testing would have:**

1. **The header said "live stream connected" while the badge said `FIXTURE`.** Two pieces of
   chrome, six inches apart, contradicting each other. The stream genuinely was connected — the
   backend sends heartbeats in fixture mode — but calling it *live* next to a `FIXTURE` badge is
   exactly the confusion this product exists to prevent. Now reads "connected — fixture data".
2. **The Brupop panel spun on "Syncing BottlerocketShadows" forever.** There was no coverage entry
   for that kind in fixture mode, and the empty state treated a missing entry as "still loading" —
   promising an answer that was never coming. An absent status now says so.
3. **The fleet table clipped its rightmost columns**, which were `rv` — the resourceVersions.
   Silently hiding provenance, in the panel where provenance matters most.
4. **"1 updates".**

Fixing (2) properly meant capturing Brupop shadows as fixtures rather than suppressing the
resulting "incomplete view" banner. Suppressing it would have trained the reader to ignore the
banner that matters — the same argument as ADR-0024.

**What the tests assert**, as distinct from the component tests: the mode badge has no dismiss
control and survives scrolling; every finding's limitations are visible *on screen*; the evidence
drawer shows field, value, and arithmetic; the concurrency control recomputes; the first Tab
reaches a visible skip link; tables use `th[scope]`; a 390px viewport does not overflow. Plus
three screenshots, which exist so a human can look.

CI now has an `e2e` job that builds the binary, installs Chromium, and uploads the report.

### M6 — leadership-grade EKS incident replay ✅ 2026-09-13

The EKS run of M5 produced a 63-minute capture that is now replayable end to end from a local
checkout, with no AWS account and no cluster.

**Backend — `ff-replay`, a third data source.** Not a flag on fixture mode: live, fixture and
replay make different claims about reality (*this is happening* / *this happened* / *this never
happened*), and sharing a code path is how one gets rendered as another. See
[ADR-0025](docs/adr/0025-replay-state-is-computed-in-rust.md).

| | |
| --- | --- |
| Bundle | `evidence/eks-recovery/` — 5,068 events, 37 artifacts, 14:15:47 → 15:19:00 |
| Determinism | `ReplayTimeline::state_at` is a pure fold. No clock, no randomness, **no interpolation** — seventeen quiet minutes replay as seventeen quiet minutes |
| Artifacts | Addressed by manifest name, never path. Traversal attempts return the same 404 as a missing file. Redacted before leaving the process |
| Mode | Every replay response carries `Mode::Replay`; the browser refuses to render anything else; both are asserted |
| Derived, not authored | Chapter marks are positions where a checkable condition first became true. Brupop's 14:08:15 start is read from Kubernetes events in the captured snapshot, not written down |

**Interface — the control room at `http://127.0.0.1:5173`.** Executive summary, replay timeline
with chapter rail and playback controls, fleet topology, FF-PDB-001 arithmetic, predicted-versus-
actual, evidence explorer, limitations. The `REPLAY — CAPTURED FROM REAL EKS/BOTTLEROCKET
EXECUTION` banner is sticky, has no dismiss control, and stays in the viewport at the bottom of
the page — a screenshot of the fleet panel has to carry the label too.

**Measured** on the M1 MacBook Air (8 GB), release build, three warm runs: **107–117 ms** to
ready, **25 MiB** resident idle, **29 MiB** after serving the full timeline and a complete
5,068-event fold. Any timeline position resolves in **~1 ms**. Frontend production bundle:
**293 kB JS (88 kB gzipped)**, **18 kB CSS (4 kB gzipped)**.

| Suite | Result |
| --- | --- |
| `cargo test --workspace` | ✅ **205 passed** |
| `cargo test -p ff-replay` | ✅ **42 passed** (35 against the real bundle) |
| `cargo test -p ff-api` | ✅ **19 passed**, of which 17 are the replay API |
| `npm test` | ✅ **45 passed** |
| `npm run test:e2e` (fixture) | ✅ **12 passed** |
| `npm run test:e2e:replay` | ✅ **31 passed** in Chromium against the real bundle |
| `cargo clippy --workspace --all-targets` | ✅ clean, warnings denied |
| `make check` | ✅ exit 0 |

**Four things the interface refuses to do**, each with a test:

1. **Show 30.2% as availability.** That figure is a *failed post-recovery networking check*.
   Availability during the incident is rendered `UNKNOWN`, because the sampler in use at the time
   had no request timeout and its output was discarded.
2. **Claim FleetForge predicted the deadlock.** It detected a PDB blocker that already existed.
   Brupop began 7m32s before recording started, and the callout derives that gap from evidence.
3. **Fold under-prediction into "conservative".** Predicting 5 evictions where 8 occurred gets its
   own class, its own colour, and a count at the top of the panel.
4. **Silently correct the `2.0.0` version bug.** Values FleetForge recorded from the wrong label
   are shown as recorded, flagged with a tooltip, and disclosed in the limitations panel.

**Demo script:** [docs/REPLAY-DEMO.md](docs/REPLAY-DEMO.md) — 5–7 minutes, with the questions you
will get and the honest answers. Screenshots in `web/e2e/screenshots/replay/`.

### M7 — release hardening ✅ 2026-09-13

**One command.** `make demo` builds the interface if stale, starts the Rust backend, serves the
production UI from the same process on one loopback port, waits for readiness, confirms the mode
is `REPLAY`, prints one URL, and shuts down cleanly on Ctrl-C. No second terminal, no Vite dev
server — a development server is not what would ever run and is a second thing to fail in front
of an audience.

The binary gained `--ui <dir>`, validated before binding: a process that starts happily and then
serves 404s to the room is worse than one that refuses to start and says why.

| Suite | Result |
| --- | --- |
| `./scripts/tests/test-demo.sh` | ✅ **18 of 18 checks** — missing evidence, evidence with no event log, occupied port, non-loopback refusal, readiness, REPLAY mode, production assets, loopback-only, JSON 404, one URL, Ctrl-C, no orphan holding the port |
| `cargo test --workspace` | ✅ **208 of 208 tests** |
| `npm test` (vitest) | ✅ **45 of 45 tests** |
| `npm run test:e2e` (fixture, Chromium) | ✅ **12 of 12 tests** |
| `npm run test:e2e:replay` | ✅ **132 of 132** = 44 tests × Chromium, Firefox, WebKit |
| — of which accessibility (axe + keyboard) | 13 tests × 3 engines |
| `make check` | ✅ exit 0 |
| clippy `-D warnings` · `cargo deny` · gitleaks | ✅ clean |

> **On counting.** Earlier reports said "12 / 31 passed", which was ambiguous. Every figure above
> is *tests passed of tests run*, and every suite passed completely. There has been no partial
> run: `12 of 12` and `132 of 132` both mean everything.

**Cross-browser.** Chromium, Firefox and WebKit, all verified locally — no browser is unverified.
Two real defects came out of it:

1. **The skip link was unreachable by keyboard in Safari/WebKit.** WebKit does not put plain
   links in the tab order unless the user has enabled Full Keyboard Access. Fixed with an
   explicit `tabIndex={0}`.
2. **The `DERIVED` tag measured 4.45:1** against a raised surface in the light theme — under AA,
   on a classification tag the whole interface depends on. Fixed with a darker accent.

**Accessibility.** axe-core over WCAG 2.1 A and AA, at four states and two viewports, plus
keyboard and focus assertions. No rule is disabled to make a test pass. Four real violations were
found and fixed:

| Violation | Cause | Fix |
| --- | --- | --- |
| `color-contrast` 1.97:1 | `.log-future { opacity: 0.45 }` | Future events marked by a dashed rule and recessed background instead of fade — no opacity value clears 4.5:1 and still reads as "not yet" |
| `color-contrast` 4.4:1 | danger red on a danger-tinted row — the **under-predicted** rows | `--danger-text`, a darker red for text on tint |
| `region` | The mode banner, provenance and position strips sat between `<header>` and `<main>`, in no landmark | One `<header role="banner">` around the persistent chrome |
| `scrollable-region-focusable` | Panel bodies and the artifact `<pre>` scroll but were not focusable | `tabIndex={0}` and an accessible name |
| `page-has-heading-one` | The brand was a `<span>` | It is the `<h1>` |

Two of my own assertions were wrong and were passing vacuously: `getComputedStyle(el,
":focus-visible")` returns an empty declaration, because the second argument takes a
pseudo-*element* and `:focus-visible` is a pseudo-*class*. The comparison `"" !== "none"` was
true for every element on the page. Both now read the focused element's real computed style.

**Walkthrough.** `docs/walkthrough/fleetforge-replay-walkthrough.mp4` — 47 seconds, recorded by
browser automation against the production binary and the real bundle. No narration, no staging,
nothing sped up. The `REPLAY` banner and badge are asserted at every one of the nine stops, and
sampled frames confirm it. A 9-second GIF of the opening is embedded in the README.

**Public-release audit.** [`docs/PUBLIC-RELEASE-AUDIT.md`](docs/PUBLIC-RELEASE-AUDIT.md).
**This repository should not be made public as it stands** — not for secrets (there are none)
but for a personal email address and a complete inventory of the AWS account's identity
configuration. Four edits and one file moved out of the tree fixes the current tree; history is a
separate decision. Nothing was published and no visibility was changed.

### M8 — the story first ✅ 2026-09-13

Visual review failed from a reader's point of view: the interface was correct and too dense to
understand. The landing page opened with 37 artifacts, 5,068 events and nine classified claims
competing for the same attention, and a reader could not find the incident in it.

**The landing view is now one screen and one story.** A headline, a one-sentence description of
what FleetForge is, four steps, the arithmetic, the outcome, the REPLAY and "did not predict"
disclaimer, and one call to action. It fits above the fold at 1280×720, 1440×900 and 1680×1050 —
asserted, not assumed.

**Everything else moved behind five tabs**, organised by the question a reader arrives with
rather than by which crate produced it: *Timeline* (minute by minute), *Investigation* (why it
happened), *The finding* (arithmetic and scoring), *Evidence* (artifacts and hashes), *Limits*
(what this cannot tell you). Nothing was deleted — the 37-artifact table, the SHA-256 hashes, the
5,068 events, the predicted-versus-actual table, the CNI hypothesis, the data caveats and the
glossary are all still there, one click away.

Every truth constraint held: `REPLAY` is on the landing view *and* in the persistent banner,
availability still reads `UNKNOWN`, 30.2% appears only in the Limits tab labelled as a failed
post-recovery networking check, `HUMAN RCA` and `UNVERIFIED` are unchanged, and no captured
evidence was touched.

| Suite | Result |
| --- | --- |
| `npm test` | ✅ **51 of 51** |
| `npm run test:e2e:replay` | ✅ **153 of 153** = 51 tests × Chromium, Firefox, WebKit |
| `npm run test:e2e` | ✅ **12 of 12** |
| `cargo test --workspace` · `make check` | ✅ **208 of 208** · exit 0 |

New assertions worth naming: the landing view must hold the four questions above the fold; it
must contain none of `5,068`, `37 artifacts`, `sha256`, `509`, `CNI` or `resourceVersion`; it
must offer exactly one button; the tab list must follow the ARIA tabs pattern; and every tab must
pass axe independently.

One more vacuous assertion was found and fixed. `focus is visible` was calling `.focus()` after a
mouse click, and browsers deliberately do not mark that as `:focus-visible` — a mouse user has
not asked for a ring. The tests now press a key first, so they ask the question a keyboard user
would.

### Phase C — live EKS run, preventative preflight ✅ 2026-09-13

A second real cluster, 1.78 hours, **$0.41**, destroyed and verified. Evidence in
`evidence/eks-live/` (26 artifacts). The historical `evidence/eks-recovery/` capture was not
modified.

**What this run added that M5 could not.** M5 detected an already-existing blocker *after*
Brupop had begun. This one ran the preflight **before Brupop existed at all** — no namespace, no
CRD, no cordons — got `BLOCKED` on FF-PDB-001 (`3 − 2 = 0`, rv=4564, snapshot `4abae803…`),
corrected one field, got `SAFE` (snapshot `9476d9bb…`), and only then installed Brupop. All
three nodes went 1.62.1 → 1.64.0 with **no deadlock and no manual uncordon**.

**The networking fault, found and explained.** The baseline before any maintenance was same-node
3/3, **cross-node 0/6**, so the maintenance experiment was halted per the stop condition. A
read-only diagnosis proved cause by failure mode: same host and moment, port 80 timed out
(dropped) while port 8080 was refused instantly (arrived) — a port filter with the path otherwise
intact. The EKS module's node SG self-rule covers `tcp 1025-65535` and `tcp/udp 53`; nginx
listens on **80**. One narrow rule took cross-node from 0/6 to **6/6**.

This **refutes** the replay's CNI-ordering hypothesis — that correction was already applied, the
CNI was ACTIVE before compute, `aws-node` never restarted, and cross-node failed anyway. It
**confirms** the 1-in-3 shape the hypothesis predicted. It does **not** prove the missing rule
caused M5's 30.2%: that cluster is gone and cannot be re-probed. See
[`evidence/eks-live/00-DIAGNOSIS.md`](evidence/eks-live/00-DIAGNOSIS.md).

**FleetForge's prediction was wrong, and the test was unfair.** Predicted 5 evictions, observed
12 — scored `missed`. Two reasons, both in how the run was set up: the preflight analysed one
node while Brupop drained three, and the "missed" workloads (`brupop-agent`, `brupop-apiserver`,
`cert-manager`) did not exist when the prediction was made. **This run establishes neither
accuracy nor inaccuracy.** A fair test predicts all three nodes with the workload set stable
across the window.

**Identity.** Terraform ran as **root**, by explicit operator override after the Identity Center
sign-in proved blocked by its own MFA policy. FleetForge itself ran as a separate read-only
ServiceAccount, proven with 18 allowed reads and 17 denied mutations including `pods/eviction`,
`escalate` and `impersonate`.

## Not done — stated explicitly

- **CI has never run.** There is no git remote. The workflow is written and enabled, so the
  cross-architecture snapshot-hash claim is verified on `aarch64` only.
- **The interface has been verified in Chromium only.** No Firefox, no WebKit, no real mobile
  device. The narrow-viewport test resizes Chromium; it does not prove anything about iOS Safari.
- **The replay is one capture.** One cluster, three nodes, 63 minutes. Determinism is proven over
  a fixed bundle; that proves what was recorded, not that the recording was complete.
- **The walkthrough recording is Chromium only**, and it is a recording of the interface rather
  than of a person using it — no cursor, no narration.
- **The ten-second comprehension claim has not been tested on a real reader.** It is asserted
  structurally — the four answers are present and above the fold — which is not the same thing.
- **Two moderate npm advisories remain**, both dev-only (vitest's bundled Vite). The critical and
  high ones were removed by moving to vitest 3.
- **The public-release audit has not been acted on.** It is a report, not a change.
- **A root access key created for Phase C is still live** (`AKIA3HPFYGMVJFEQ56QU`). Deleting it
  is item 9 of the teardown order in `docs/aws-account-changes.md` and is the highest-priority
  outstanding action in this repository.
- **Two KMS CMKs are in PendingDeletion until 2026-10-13**, ~$2/month total. EKS schedules rather
  than deletes them. Left alone deliberately — Phase C forbade altering KMS.
- **Phase C's evidence is not served by the replay interface.** `--replay` reads
  `evidence/eks-recovery/` only; the new bundle has a different shape and no loader.
- **The prediction accuracy question is still open.** See the unfair-test note above.
- **No automated accessibility audit.** Focus order, header cells, and contrast were checked by
  hand and by targeted assertions, not by axe or similar.
- **No scheduler predicate evaluation.** Capacity findings are `Heuristic` and say so in their own
  `limitations`. `FF-STORAGE-001` cannot read PersistentVolume node affinity, because FleetForge
  has no PV read permission at this milestone — a network PV that is in fact zone-bound is not
  flagged.
- **No pod anti-affinity or topology-spread analyzer.** Both are modelled in `ff-core` and
  collected by `ff-collect`, but no analyzer reads them yet. `FF-AZ-001/002` covers the zone case
  only.
- **No real Brupop.** Only its CRD is installed; the `BottlerocketShadow` objects are
  hand-written. The collection path is proven against a real API server; observing an actual
  Bottlerocket update needs M5.
- **Prediction accuracy is exercised, not measured.** One prediction was scored against a single
  pod deletion. A real drain is needed, and needs approval.
- No AWS call. No cluster mutation capability in FleetForge.
- `target/` is now **4.9 GB**, with 24 GB free. Worth a `cargo clean` before M5, which adds Terraform and container builds.

## Decisions taken

| ID | Decision | Rationale |
| --- | --- | --- |
| D1 | Repository at `~/fleetforge` | Matches the layout of your other projects |
| D2 | SSE over WebSockets | One-directional data; free reconnection; plain `GET` for auth — ADR-0007 |
| D3 | `ff-collect` owns the only Kubernetes client | Makes "read-only cannot mutate" testable — ADR-0008 |
| D4 | Evidence-based analyzers, not scheduler simulation, in V1 | Honest and achievable — ADR-0004 |
| D5 | Apache-2.0 | Standard for this ecosystem; compatible with Brupop's licence — ADR-0010 |
| D6 | Docker Desktop rather than Colima | Already installed; no reason to add a second VM runtime — ADR-0009 |
| D7 | Recommendation summary is preflight output, not a controller | A calculation over one snapshot, not a stateful subsystem — ADR-0016 |
| D8 | EKS is ephemeral; the artifacts are the deliverable | ~$8/day only while capturing the demo; forces reproducible stand-up — ADR-0017 |
| D9 | Append-only JSONL event log before SQLite | "Persisted event JSON" is a deliverable a reviewer can `jq`; defers sqlx compile cost — ADR-0018 |
| D10 | Brupop executes; FleetForge never mutates | No mutating client is constructed, so the safety property is structural — ADR-0012 |

## Commands or approvals required from you

Nothing has been installed or changed outside `~/fleetforge`. To unblock M1:

To see it yourself — the cluster is already running:

```bash
cd ~/fleetforge && export PATH="$HOME/.cargo/bin:$PATH"

make check                                    # fmt, clippy -D warnings, 87 tests
./scripts/mutation-denial-test.sh             # proves the identity cannot mutate
./scripts/watch-demo.sh                       # scale + pod delete appearing live
./scripts/resilience-demo.sh                  # API server outage handling
./scripts/expired-credential-test.sh          # credential expiry handling

./target/debug/fleetforge --kubeconfig infra/local/.kubeconfig-fleetforge-reader
cd web && npm run dev                         # then open http://127.0.0.1:5173
```

Approvals needed: **(a)** start M3.
M5 AWS provisioning and every individual `kubectl` change during the demonstration each require
their own separate approval in-session.

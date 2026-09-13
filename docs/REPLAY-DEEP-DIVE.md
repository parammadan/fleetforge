# Technical deep dive

For an engineer who wants to check the claims rather than be told them. Roughly
forty-five minutes, in the order that builds on itself.

## 0. Start it

```sh
make demo          # http://127.0.0.1:8080
```

One process: the Rust binary serves both the API and the production interface.
It holds no mutating Kubernetes client and binds to loopback. It reads one
directory and serves HTTP. Nothing else.

If you want the pieces apart — say, to run the frontend with hot reload:

```sh
cargo build --release -p ff-api
./target/release/fleetforge --replay evidence/eks-recovery --bind 127.0.0.1:8080
cd web && npm ci && npm run dev          # http://127.0.0.1:5173
```

## 1. Read the evidence before reading the code

```sh
head -1 evidence/eks-recovery/35-fleetforge-events-complete.jsonl | jq
wc -l evidence/eks-recovery/35-fleetforge-events-complete.jsonl   # 5068
cat evidence/eks-recovery/00-CONCLUSIONS.md
```

Then the load-bearing fact — that Brupop started before FleetForge was watching:

```sh
# Brupop's first activity, from events already in the API server's history
jq -r '.data.events[]
       | select(.involved_object.namespace == "brupop-bottlerocket-aws")
       | .first_seen_at' evidence/eks-recovery/01-snapshot-before.json | sort | head -1
# 2026-09-13T14:08:15Z

# FleetForge's first recorded event
head -1 evidence/eks-recovery/35-fleetforge-events-complete.jsonl | jq -r .recorded_at
# 2026-09-13T14:15:47.607196Z
```

7m 32s. Everything the interface says about prediction rests on that gap, and
`crates/ff-replay/src/bundle.rs::parse_brupop_start` is the twenty lines that
recompute it on every load.

## 2. Check the arithmetic against the cluster's own words

FleetForge's finding:

```sh
jq '.data.findings[] | select(.id == "FF-PDB-001") | .calculation' \
   evidence/eks-recovery/03-preflight-before.json
```

Kubernetes' independent account of the same thing:

```sh
jq '.items[] | select(.metadata.name == "web-pdb") | .status' \
   evidence/eks-recovery/04-pdb-before.json
```

Note `conditions[].reason == "InsufficientPods"` with
`lastTransitionTime: 14:12:12Z` — the API server recorded the budget going
undisruptable three and a half minutes *before* FleetForge connected. Two
independent sources, same conclusion, neither of them this project's prose.

## 3. Prove the fold is deterministic

```sh
curl -s 'localhost:8080/api/v1/replay/state?position=2932' | sha256sum
curl -s 'localhost:8080/api/v1/replay/state?position=2932' | sha256sum   # identical
curl -s 'localhost:8080/api/v1/replay/state?position=99999' | jq .data.events_applied
# 5068 — clamped, not an error
```

`crates/ff-replay/src/state.rs` is the whole implementation: a `BTreeMap` fold
over `events[0..=position]`. No clock, no randomness, no interpolation. The last
matters most — if the cluster produced nothing between 14:36 and 14:53, the
replay shows nothing for seventeen minutes. Smoothing that would be fabricating
Kubernetes activity.

```sh
cargo test -p ff-replay the_timeline_contains_a_real_gap -- --nocapture
```

## 4. Try to break out of the artifact store

```sh
curl -s -o /dev/null -w '%{http_code}\n' 'localhost:8080/api/v1/replay/artifacts/..%2F..%2F..%2Fetc%2Fpasswd'
curl -s -o /dev/null -w '%{http_code}\n' 'localhost:8080/api/v1/replay/artifacts/%2Fetc%2Fpasswd'
curl -s -o /dev/null -w '%{http_code}\n' 'localhost:8080/api/v1/replay/artifacts/.%2F00-CONCLUSIONS.md'
# 404, 404, 404 — same as a name that simply is not in the manifest
```

`crates/ff-replay/src/artifacts.rs` indexes the directory once, canonicalizes,
excludes dotfiles, and looks names up in a `BTreeMap`. There is no path
concatenation anywhere in the read path, so there is nothing for `..` to do.

Redaction is wired into the HTTP path, not just unit-tested:

```sh
cargo test -p ff-api artifact_contents_are_redacted_on_the_way_out
cargo test -p ff-api the_real_bundle_serves_no_credential_material
```

## 5. Prove replay cannot claim to be live

```sh
for p in context timeline state chapters chain claims finding predictions traffic artifacts; do
  printf '%-12s %s\n' "$p" "$(curl -s localhost:8080/api/v1/replay/$p | jq -r .mode)"
done
curl -s localhost:8080/api/v1/environment | jq -r .mode_label   # REPLAY
```

Three independent guards — the Rust envelope, `no_replay_endpoint_can_report_live`,
and the browser's own refusal in `getEnvelope()`. The last has an E2E test that
rewrites a response to `mode: "live"` and asserts the screen shows an error
instead of the data:

```sh
cd web && npx playwright test -c playwright.replay.config.ts -g "mode other than replay"
```

## 6. Read the scoring, then disagree with it

```sh
curl -s localhost:8080/api/v1/replay/predictions | jq -r \
  '.data[] | "\(.at[11:19]) pred=\(.predicted) obs=\(.observed) \(.class)"'
```

Two rows read `pred=16 obs=0` and score differently. The difference is
`window_had_activity`, and the interface shows that column rather than leaving
the table looking inconsistent.

`crates/ff-record/src/score.rs::verdict` is forty lines and worth reading in
full. An earlier version returned "conservative" whenever no *workload* was
missed, which labelled predicted-5/observed-8 as conservative. Over- and
under-prediction are not the same mistake: one wastes an operator's caution, the
other spends it somewhere it was needed.

## 7. The chain, and what FleetForge does not do

```sh
curl -s localhost:8080/api/v1/replay/chain | jq -r \
  '.data.links[] | "\(.basis)\t\(.value)"; .data.edges[] | "\(.basis)\t\(.from)→\(.to)"'
```

Five facts, all evidence. Four arrows, all `human_rca`. Then confirm the claim
that FleetForge implements no such correlation:

```sh
grep -rn "unschedulable" crates/ff-preflight/src/analyzers/ | grep -i "pdb\|disruption"
# no analyzer joins cordon state to PodDisruptionBudget status
ls crates/ff-preflight/src/analyzers/
```

The chain is in `crates/ff-replay/src/chain.rs`, and its values are read from the
fold and from `04-pdb-before.json` — not typed in. Damage the evidence and the
prose changes:

```sh
cargo test -p ff-replay chain_values_come_from_the_bundle_rather_than_from_constants
cargo test -p ff-replay a_bundle_whose_pdb_lost_its_status_is_rejected
```

## 8. Run everything

```sh
make check                    # fmt, clippy -D warnings, tests, shell tests, demo tests
cargo test --workspace        # 208 of 208

cd web
npm test                      # 45 of 45  (vitest)
npm run test:e2e              # 12 of 12  (fixture mode, Chromium)
npm run test:e2e:replay       # 132 of 132 = 44 tests × Chromium, Firefox, WebKit
npm run test:e2e:a11y         # the axe subset of the above
npm run walkthrough           # re-record docs/walkthrough/
```

Every figure is *passed of run*. There is no partially-passing suite.

## Where the interesting code is

| Question | File |
| --- | --- |
| What is a claim, and what makes it evidence? | `crates/ff-replay/src/schema.rs` |
| How is state at a position computed? | `crates/ff-replay/src/state.rs` |
| What is parsed out of the bundle, and what is rejected? | `crates/ff-replay/src/bundle.rs` |
| How are chapters derived rather than authored? | `crates/ff-replay/src/chapters.rs` |
| Why are the chain's arrows weaker than its boxes? | `crates/ff-replay/src/chain.rs` |
| How is path traversal prevented? | `crates/ff-replay/src/artifacts.rs` |
| Where does the mode label come from? | `crates/ff-api/src/routes.rs::replay_envelope` |
| Why can't the browser invent state? | `web/src/useReplay.ts` |
| Why is under-prediction its own class? | `crates/ff-record/src/score.rs` |
| How does one command become one URL? | `scripts/demo.sh`, `crates/ff-api/src/ui.rs` |

## The three bugs worth knowing about

Each is disclosed in the interface rather than cleaned up.

1. **`bottlerocket_version: "2.0.0"`.** FleetForge read the
   `bottlerocket.aws/updater-interface-version` label into the release field.
   Fixed partway through the capture; the bad values stay, flagged with a
   tooltip and a data caveat.
2. **The traffic sampler had no timeout.** `wget` with no `-T` blocked for
   minutes per failure, producing 31 samples in 34 minutes instead of ~2,000.
   Its output was discarded, which is why availability reads
   `UNKNOWN DURING INCIDENT` rather than a number.
3. **Under-prediction was labelled "conservative".** Corrected in the scorer, and
   the correction is why two rows now carry the red class.

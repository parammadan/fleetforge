# Leadership demo — EKS incident replay

Five to seven minutes. One screen, no slides, no live cluster, nothing to go wrong on stage.

Everything shown is a recording of a real Amazon EKS cluster running real Bottlerocket nodes
under Brupop on 13 September 2026. The cluster has been destroyed. The recording has not.

---

## Before you start

```sh
make demo
```

That is the whole thing. It builds the interface if it is stale, starts the backend, serves the
production UI from the same process, waits until it is genuinely ready, checks the mode is
`REPLAY`, and prints one URL:

```
  FleetForge replay  →  http://127.0.0.1:8080
```

Ctrl-C stops it. One terminal, one process, nothing left running.

If 8080 is taken it says so, names what is holding it, and tells you to use
`FLEETFORGE_PORT=8081 make demo`. If the evidence bundle is missing it refuses to start rather
than showing an empty control room.

Measured on an M1 MacBook Air (8 GB), release build, over three warm runs: ready in
**107–117 ms**, **25 MiB** resident idle and **29 MiB** after serving the full timeline and a
complete 5,068-event fold. Any position on the timeline resolves in **~1 ms**. The frontend
production bundle is **293 kB of JavaScript (88 kB gzipped)** and **18 kB of CSS (4 kB
gzipped)**. Nothing is pre-computed and nothing is cached between requests.

First run after a build is slower — around 700–850 ms — because the binary is not yet in the
page cache.

If the backend is not running, the page says so and shows nothing else. That is deliberate.

---

## 0:00 — What you are looking at (45 seconds)

*Point at the grey band under the header. Leave it there; it does not go away and there is no
way to close it.*

> This is a replay. It was captured from a real EKS cluster with Bottlerocket worker nodes,
> being updated by Brupop — the Bottlerocket update operator. Sixty-three minutes, 5,068
> events, 37 artifacts. The cluster is gone. Every number you are about to see came out of
> that recording, and none of it has been cleaned up.

*Point at the four tiles.*

> Four numbers a leader would ask for. Look at the first one.

---

## 0:45 — The number we do not have (75 seconds)

**Availability during the incident: UNKNOWN.**

> This is the number everybody wants and we do not have it. The traffic sampler running
> during the incident had no request timeout. When the service stopped answering, the sampler
> sat there waiting — so what it recorded was how patient `wget` is, not how available the
> service was. It produced 31 samples in 34 minutes. We threw the data away.
>
> We could have put the 30.2% from the fourth tile here instead. Look at what that number
> actually is.

*Point at the fourth tile: **FAILED · 30.2%**.*

> That is a post-recovery networking check that **failed**. It ran after everything was back,
> and 42 of 139 requests succeeded. Labelling it "availability" would have made this incident
> look like a 30% outage. It is not that. It is a separate networking problem we found while
> validating the fix.
>
> The tile says UNKNOWN because UNKNOWN is the answer. A dashboard that cannot say "we don't
> know" will eventually say something worse.

**Why this lands:** it is the moment the audience learns the screen is not selling to them.

---

## 2:00 — What FleetForge did, and what it did not do (60 seconds)

*Point at the blue callout under the tiles.*

> FleetForge did not predict this. Brupop started updating the fleet at 14:08:15. FleetForge
> started recording at 14:15:47 — 7m 32s later. By the time it was
> watching, two of the three nodes were already cordoned.
>
> That timestamp is not written into the interface. It is read out of the Kubernetes events
> captured in the pre-incident snapshot, every time the bundle loads.

*Scroll to the claim list.*

> Every statement on this screen carries a tag saying where it came from. Green **OBSERVED**
> means FleetForge watched it happen and it is in the event log. Purple **HUMAN RCA** means a
> person worked it out afterwards. Amber **UNVERIFIED** means plausible but never tested. Grey
> **NO EVIDENCE** means the data does not exist.
>
> Only two of those five are evidence. The interface will not let you mistake the other three
> for it.

---

## 3:00 — Replay the incident (2 minutes)

*Click the chapter **14:15:50 — FleetForge reports BLOCKED**.*

> Here is the first preflight. BLOCKED, on finding FF-PDB-001.

*Scroll down to the FF-PDB-001 panel.*

> It is not a verdict, it is arithmetic you can check. `disruptionsAllowed = currentHealthy −
> desiredHealthy`. Two minus two is zero. And under it, the exact field each number was read
> from — `.status.disruptionsAllowed`, `.status.currentHealthy`, `.spec.minAvailable`. If you
> disagree with the conclusion you can find the number you disagree with.
>
> Below that, what the finding *cannot* tell you. It reflects one moment; a pod becoming ready
> changes it. It covers voluntary eviction only; a node that dies outright ignores
> PodDisruptionBudgets entirely.

*Scroll back to the timeline. Click the chapter **The loop is already in place**.*

> Two nodes cordoned, one pod stuck Pending — in the first state FleetForge ever saw. The
> deadlock was not forming. It had already formed.
>
> The chain is tagged HUMAN RCA, and that matters: the cordons left nowhere to schedule the
> third web replica, which held currentHealthy at two, which held disruptionsAllowed at zero,
> which blocked the eviction Brupop needed to proceed. A person read node, pod and PDB state
> together and drew that line. FleetForge reported the blocker. It does not implement that
> inference and does not claim to.

*Click **14:35:11 — First manual uncordon**, then watch the strip under the banner.*

> A human uncordons a node with `kubectl`. FleetForge holds no mutating client and issued
> nothing here — it watched.

*Click **14:53:06 — Second manual uncordon**. Point at the strip: `0 cordoned`.*

> Second uncordon, scheduling resumes, Brupop finishes. All three nodes reach Bottlerocket
> 1.64.0.

**If someone asks about the ⚠ next to a version number:** FleetForge read the Bottlerocket
version from the wrong label early in the capture and recorded `2.0.0`, which is not a
release. The bug was fixed partway through. The bad values are still on screen, flagged, because
editing captured evidence to look better is the one thing this system must not do.

---

## 5:00 — What it got wrong (60 seconds)

*Scroll to **Predicted versus actual**.*

> Seven preflights, five actually tested. Two of them **under-predicted** — FleetForge said
> five evictions and eight happened, said sixteen and twenty-one happened.
>
> Under-prediction has its own row colour and its own count at the top of the panel. An
> earlier version of the scorer called this "conservative", because no workload was missed.
> That was wrong and it was the flattering kind of wrong: over-prediction wastes an operator's
> caution, under-prediction spends it somewhere it was needed. They are not the same mistake
> and they no longer share a label.

*Point at the two rows that both read 16 / 0.*

> These two rows have identical numbers and different verdicts. The "disruption in window"
> column is why: in one, nothing was drained at all, so the prediction was never tested — and
> untested is not a pass.

---

## 6:00 — Close (45 seconds)

*Scroll to the bottom panel.*

> Last panel is what this cannot tell you. Three data caveats about the evidence itself, every
> unverified claim repeated so it survives skim-reading, and the scope:
>
> One cluster, three nodes, one hour. FleetForge observed and explained; Brupop executed.
> Nothing here demonstrates safe execution, because FleetForge never executed anything — it
> has no mutating Kubernetes client at all.
>
> That is the product. It analyses, explains, plans, observes and audits. Brupop does the
> work. This screen is what it looks like when a tool refuses to tell you more than it knows.

---

## Questions you will get

**"Why not just show the 30.2% as availability?"**
Because it is a failed post-recovery networking check, measured after everything was back. It
would make a deadlock look like a 30% outage and it would be false. See the traffic tile and
`evidence/eks-recovery/31-traffic-post-recovery.txt`.

**"Would FleetForge have prevented this?"**
Unknown, and the interface will not claim otherwise. FleetForge detected the PDB blocker
within three seconds of connecting. Whether it would have caught it *before* Brupop started —
it was not running then — is untested.

**"Is the 1-in-3 traffic result a CNI problem?"**
Tagged UNVERIFIED. A 1-in-3 success rate against a three-endpoint Service is consistent with
only the same-node endpoint being reachable, which would point at the VPC CNI having been
installed after the nodes joined. No packet capture was taken and no controlled comparison was
run. The Terraform correction is written; it is not verified on a fresh cluster, because the
cluster was destroyed first.

**"Can we see the raw data?"**
Yes — the Evidence panel. 37 artifacts, each with the SHA-256 it had when captured. Click any
of them. They are served through a redactor and addressed by manifest name, so the panel
cannot be used to read arbitrary files.

**"Is this live?"**
No, and it cannot pretend to be. Every response from the replay backend carries `mode:
replay`; the browser refuses to render anything labelled otherwise; and a test asserts both.

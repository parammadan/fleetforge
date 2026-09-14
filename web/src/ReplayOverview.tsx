// The landing view: the whole incident in one screen.
//
// The rule this file exists to enforce is that somebody who knows Kubernetes
// but has never heard of FleetForge can answer four questions in ten seconds —
// what broke, why it was blocked, how it was recovered, and what FleetForge
// actually did. Everything that serves a second question goes below the fold.
//
// What was moved out, and why: the 37-artifact table, SHA-256 hashes, 5,068
// events, the predicted-versus-actual table, the CNI hypothesis, the evidence
// caveats and the provenance block are all *proof*. Proof is what you reach for
// after you believe the claim, and putting it first means the claim never
// lands. None of it is gone; it is one click away and one section down.
//
// What stayed: the REPLAY label, the "did not predict" disclaimer, and the
// arithmetic. Those are not detail — they are the parts most likely to be
// misread if a reader skims, so they are on the first screen by design.

import type { CaptureContext, CaptureKind, PdbArithmetic, ReplayState } from "./replayTypes";
import { Term } from "./glossary";

export interface OverviewProps {
  kind: CaptureKind;
  context: CaptureContext;
  pdb: PdbArithmetic;
  /** State at the end of the capture, for the outcome. */
  finalState: ReplayState | null;
  nodeCount: number;
  onExplore: () => void;
}

/** One box in the incident chain. */
function Step({
  n,
  title,
  detail,
  tone,
}: {
  n: number;
  title: React.ReactNode;
  detail: React.ReactNode;
  tone: "neutral" | "bad" | "good";
}) {
  return (
    <li className={`story-step story-${tone}`}>
      <span className="story-n" aria-hidden="true">
        {n}
      </span>
      <h3>{title}</h3>
      <p>{detail}</p>
    </li>
  );
}

export function ReplayOverview({
  kind,
  context,
  pdb,
  finalState,
  nodeCount,
  onExplore,
}: OverviewProps) {
  if (kind === "prevented") {
    return (
      <PreventedOverview
        context={context}
        pdb={pdb}
        finalState={finalState}
        nodeCount={nodeCount}
        onExplore={onExplore}
      />
    );
  }
  const updated = finalState
    ? finalState.nodes.filter((n) => n.bottlerocket_version === "1.64.0").length
    : nodeCount;

  // Derived, not written: if a future bundle had a different gap, the sentence
  // changes with it rather than staying wrong.
  const gapSeconds = context.brupop_first_seen_at
    ? Math.floor(
        (Date.parse(context.captured_from) - Date.parse(context.brupop_first_seen_at)) / 1000,
      )
    : null;
  const gap =
    gapSeconds === null ? null : `${Math.floor(gapSeconds / 60)}m ${gapSeconds % 60}s`;

  return (
    <section className="overview" aria-labelledby="overview-headline">
      <div className="overview-inner">
        <p className="overview-kicker">Amazon EKS · Bottlerocket · Brupop · 13 September 2026</p>

        <h2 id="overview-headline" className="overview-headline">
          A routine operating-system update got stuck, and the cluster's own safety rule was
          what stopped it.
        </h2>

        <p className="overview-what">
          <strong>FleetForge</strong> reads a Kubernetes cluster and explains why maintenance is
          blocked — showing the arithmetic behind every answer. It does not drain, update or
          reboot anything.
        </p>

        <ol className="story" aria-label="What happened, in four steps">
          <Step
            n={1}
            tone="neutral"
            title="The update began"
            detail={
              <>
                During the <Term k="brupop" /> update, two of three nodes were{" "}
                <Term k="cordon">cordoned</Term> — marked unavailable for new work.
              </>
            }
          />
          <Step
            n={2}
            tone="bad"
            title="A replica had nowhere to go"
            detail={
              <>
                With two nodes out, the third copy of the <code>web</code> app could not be
                placed. It sat <Term k="pending">Pending</Term>, so only 2 of 3 were healthy.
              </>
            }
          />
          <Step
            n={3}
            tone="bad"
            title="Kubernetes refused the next eviction"
            detail={
              <>
                The <Term k="pdb">PodDisruptionBudget</Term> required 2 healthy. With 2 healthy
                and 2 required, nothing could be taken down — so the update could not proceed.
              </>
            }
          />
          <Step
            n={4}
            tone="good"
            title="A human cleared it"
            detail={
              <>
                Two manual <code>kubectl uncordon</code> calls freed capacity. Scheduling
                resumed and the update finished.
              </>
            }
          />
        </ol>

        <div className="overview-result">
          <figure className="sum" aria-label="The blocker, as arithmetic">
            <figcaption>Why it was blocked</figcaption>
            <div className="sum-row">
              <span className="sum-term">
                <b>2</b>
                <small>healthy</small>
              </span>
              <span className="sum-op" aria-hidden="true">
                −
              </span>
              <span className="sum-term">
                <b>2</b>
                <small>required</small>
              </span>
              <span className="sum-op" aria-hidden="true">
                =
              </span>
              <span className="sum-term sum-zero">
                <b>{pdb.result}</b>
                <small>evictions allowed</small>
              </span>
            </div>
            <p className="sum-note">
              <code>{pdb.finding_id}</code> · observed from the real cluster during capture
            </p>
          </figure>

          <figure className="outcome" aria-label="Outcome">
            <figcaption>Outcome</figcaption>
            <strong className="outcome-value">
              {updated}/{nodeCount}
            </strong>
            <p>nodes updated to Bottlerocket 1.64.0</p>
            <p className="outcome-note">
              Customer impact during the incident is <b>UNKNOWN</b> — the traffic sampler in use
              was defective and its data was discarded.
            </p>
          </figure>
        </div>

        <p className="overview-disclaimer">
          <span className="disclaimer-tag">REPLAY</span>
          <span>
            Captured from a real EKS/Bottlerocket execution. Nothing here is live.{" "}
            <strong>FleetForge did not predict this incident</strong> — the update was already
            underway{gap ? ` for ${gap}` : ""} and two nodes were already cordoned before
            recording started. It detected and explained the blocker; it did not foresee it.
          </span>
        </p>

        <button type="button" className="cta" onClick={onExplore}>
          Explore the incident
          <span aria-hidden="true"> ↓</span>
        </button>
        <p className="cta-note">
          Timeline, evidence, the arithmetic, and everything FleetForge got wrong.
        </p>
      </div>
    </section>
  );
}

/**
 * The landing view for a prevented run.
 *
 * Deliberately a separate component rather than the incident view with the
 * words swapped. The two make close to opposite claims — one admits it did not
 * predict, the other says it stopped the thing before it started — and a single
 * template with conditional strings is how those get quietly interchanged.
 *
 * The claim this screen makes is stronger than the incident's, so it is held to
 * a stricter standard: it states the ordering that earns it, and it carries the
 * prediction failure on the same screen rather than behind a tab.
 */
function PreventedOverview({
  context,
  pdb,
  finalState,
  nodeCount,
  onExplore,
}: Omit<OverviewProps, "kind">) {
  const updated = finalState
    ? finalState.nodes.filter((n) => n.bottlerocket_version === "1.64.0").length
    : nodeCount;

  return (
    <section className="overview" aria-labelledby="overview-headline">
      <div className="overview-inner">
        <p className="overview-kicker">
          Amazon EKS · Bottlerocket · Brupop · {context.captured_from.slice(0, 10)}
        </p>

        <h2 id="overview-headline" className="overview-headline">
          The same failure, caught before it happened.
        </h2>

        <p className="overview-what">
          <strong>FleetForge</strong> checked the cluster before the update operator was even
          installed, refused it, and named the one field to change. It does not drain, update
          or reboot anything — it reads, and it explains.
        </p>

        <ol className="story" aria-label="What happened, in four steps">
          <Step
            n={1}
            tone="bad"
            title="A budget with no slack"
            detail={
              <>
                A <Term k="pdb">PodDisruptionBudget</Term> requiring all three replicas — so
                nothing could be taken down. The condition was set deliberately, to test it.
              </>
            }
          />
          <Step
            n={2}
            tone="bad"
            title="FleetForge said BLOCKED"
            detail={
              <>
                Before <Term k="brupop" /> existed in the cluster: no namespace, no{" "}
                <Term k="cordon">cordons</Term>, nothing running. It named the finding and
                showed the arithmetic.
              </>
            }
          />
          <Step
            n={3}
            tone="neutral"
            title="One field changed"
            detail={
              <>
                <code>minAvailable</code> 3 → 2. A second check returned <b>SAFE</b> against a
                different <Term k="snapshot">snapshot hash</Term>. No pod restarted.
              </>
            }
          />
          <Step
            n={4}
            tone="good"
            title="The update just ran"
            detail={
              <>
                Brupop cordoned, drained, rebooted and restored all three nodes. No deadlock,
                no manual intervention.
              </>
            }
          />
        </ol>

        <div className="overview-result">
          <figure className="sum" aria-label="The blocker, as arithmetic">
            <figcaption>What it caught</figcaption>
            <div className="sum-row">
              <span className="sum-term">
                <b>3</b>
                <small>healthy</small>
              </span>
              <span className="sum-op" aria-hidden="true">
                −
              </span>
              <span className="sum-term">
                <b>3</b>
                <small>required</small>
              </span>
              <span className="sum-op" aria-hidden="true">
                =
              </span>
              <span className="sum-term sum-zero">
                <b>{pdb.result}</b>
                <small>evictions allowed</small>
              </span>
            </div>
            <p className="sum-note">
              <code>{pdb.finding_id}</code> · observed from the real cluster during capture
            </p>
          </figure>

          <figure className="outcome" aria-label="Outcome">
            <figcaption>Outcome</figcaption>
            <strong className="outcome-value">
              {updated}/{nodeCount}
            </strong>
            <p>nodes updated to Bottlerocket 1.64.0</p>
            <p className="outcome-note">
              And FleetForge's eviction <b>prediction was wrong</b> — it said 5, twelve
              happened. The test was not a fair one; see Limits.
            </p>
          </figure>
        </div>

        <p className="overview-disclaimer">
          <span className="disclaimer-tag">REPLAY</span>
          <span>
            Captured from a real EKS/Bottlerocket execution. Nothing here is live. The claim
            that this was <strong>prevented rather than explained</strong> rests on one thing
            and is worth checking: the BLOCKED result is timestamped 22:40:19, and the first
            trace of Brupop anywhere in the log is 22:44:02.
          </span>
        </p>

        <button type="button" className="cta" onClick={onExplore}>
          Explore the experiment
          <span aria-hidden="true"> ↓</span>
        </button>
        <p className="cta-note">
          The timeline, the arithmetic, the networking fault found along the way, and what
          FleetForge got wrong.
        </p>
      </div>
    </section>
  );
}

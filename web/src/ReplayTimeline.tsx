// The timeline: chapter rail, scrubber, playback controls, event log.
//
// The scrubber runs over *significant* events, not all 5,068. A slider whose
// full travel is 5,068 steps moves roughly nineteen events per pixel on a
// laptop, which makes it impossible to land on a specific moment — and landing
// on specific moments is the only thing a demo scrubber is for.

import { useEffect, useRef } from "react";
import type { Chapter, ReplayEvent, ReplayState } from "./replayTypes";
import type { Playback, Speed } from "./useReplay";
import { SPEEDS } from "./useReplay";
import { BasisTag, Panel, timeOf } from "./ReplayPanels";

export function ReplayTimeline({
  timeline,
  chapters,
  playback,
  totalEvents,
}: {
  timeline: ReplayEvent[];
  chapters: Chapter[];
  playback: Playback;
  totalEvents: number;
}) {
  const current = timeline[playback.step];
  const chapter = currentChapter(chapters, playback.position);

  return (
    <Panel
      id="timeline"
      title="Replay timeline"
      subtitle={`step ${playback.step + 1} of ${timeline.length} · event ${
        playback.position + 1
      } of ${totalEvents}`}
    >
      <ChapterRail
        chapters={chapters}
        timeline={timeline}
        step={playback.step}
        onSeek={playback.seekToPosition}
      />

      {chapter && (
        <div className="chapter-note">
          <div className="claim-head">
            <strong>{chapter.title}</strong>
            <BasisTag basis={chapter.basis} />
            <span className="mono claim-at">{timeOf(chapter.at)}</span>
          </div>
          <p>{chapter.narration}</p>
        </div>
      )}

      <Controls playback={playback} timeline={timeline} />

      <EventLog timeline={timeline} step={playback.step} onSeek={playback.seekToStep} />

      {current && (
        <p className="claim-evidence">
          Showing state after event {current.seq} at {timeOf(current.at)}. Gaps between
          events are real: the replay does not interpolate, so a quiet cluster stays quiet.
        </p>
      )}
    </Panel>
  );
}

function currentChapter(chapters: Chapter[], position: number): Chapter | undefined {
  let found: Chapter | undefined;
  for (const c of chapters) {
    if (c.position <= position) found = c;
    else break;
  }
  return found;
}

/**
 * Marks are placed by *step*, not by raw event index.
 *
 * The capture is wildly non-uniform in time: 731 of its 5,068 events land in
 * the first three seconds, because that is the initial snapshot arriving. Laid
 * out by raw index, the first four chapters pile up in the leftmost 14% of the
 * rail and cannot be clicked apart. Laid out by step, they match where the
 * scrubber will actually go — which is the only thing a mark is promising.
 */
function stepOf(timeline: ReplayEvent[], position: number): number {
  let best = 0;
  for (let i = 0; i < timeline.length; i += 1) {
    const entry = timeline[i];
    if (entry && entry.index <= position) best = i;
    else break;
  }
  return best;
}

function ChapterRail({
  chapters,
  timeline,
  step,
  onSeek,
}: {
  chapters: Chapter[];
  timeline: ReplayEvent[];
  step: number;
  onSeek: (position: number) => void;
}) {
  const last = Math.max(1, timeline.length - 1);
  const pct = (s: number) => (s / last) * 100;

  return (
    <nav className="chapter-rail" aria-label="Chapters">
      <div className="rail-track">
        <div className="rail-progress" style={{ width: `${pct(step)}%` }} />
        {chapters.map((c) => {
          const at = stepOf(timeline, c.position);
          return (
            <button
              key={c.id}
              type="button"
              className={`rail-mark ${at <= step ? "rail-mark-past" : ""}`}
              style={{ left: `${pct(at)}%` }}
              onClick={() => onSeek(c.position)}
              title={`${c.title} — ${timeOf(c.at)}`}
              aria-label={`Jump to ${c.title} at ${timeOf(c.at)}`}
            />
          );
        })}
      </div>
      <ol className="chapter-list">
        {chapters.map((c) => (
          <li key={c.id}>
            <button
              type="button"
              className={`chapter-button ${
                stepOf(timeline, c.position) <= step ? "chapter-past" : ""
              }`}
              onClick={() => onSeek(c.position)}
            >
              <span className="mono">{timeOf(c.at)}</span>
              {c.title}
            </button>
          </li>
        ))}
      </ol>
    </nav>
  );
}

function Controls({ playback, timeline }: { playback: Playback; timeline: ReplayEvent[] }) {
  return (
    <div className="controls">
      <button
        type="button"
        onClick={playback.playing ? playback.pause : playback.play}
        className="control-primary"
        disabled={timeline.length === 0}
      >
        {playback.playing ? "Pause" : "Play"}
      </button>
      <button type="button" onClick={playback.stepBack} disabled={playback.step === 0}>
        ◀ Step
      </button>
      <button
        type="button"
        onClick={playback.stepForward}
        disabled={playback.step >= timeline.length - 1}
      >
        Step ▶
      </button>
      <button type="button" onClick={playback.restart}>
        Restart
      </button>

      <span className="speed" role="group" aria-label="Playback speed">
        {SPEEDS.map((s: Speed) => (
          <button
            key={s}
            type="button"
            className={playback.speed === s ? "speed-active" : ""}
            onClick={() => playback.setSpeed(s)}
            aria-pressed={playback.speed === s}
          >
            {s}×
          </button>
        ))}
      </span>

      <label className="scrub">
        <span className="visually-hidden">Timeline position</span>
        <input
          type="range"
          min={0}
          max={Math.max(0, timeline.length - 1)}
          value={playback.step}
          onChange={(e) => playback.seekToStep(Number(e.target.value))}
          aria-valuetext={
            timeline[playback.step]
              ? `${timeOf(timeline[playback.step]?.at ?? "")} — ${
                  timeline[playback.step]?.summary ?? ""
                }`
              : undefined
          }
        />
      </label>
    </div>
  );
}

/** The log, scrolled so the current entry stays in view during playback. */
function EventLog({
  timeline,
  step,
  onSeek,
}: {
  timeline: ReplayEvent[];
  step: number;
  onSeek: (step: number) => void;
}) {
  const activeRef = useRef<HTMLLIElement>(null);
  const listRef = useRef<HTMLOListElement>(null);

  // `scrollIntoView` walks up to every scrollable ancestor, including the
  // document — on load it dragged the whole page past the executive summary to
  // put row zero in view. Scroll the container directly instead, so the log
  // follows the playhead without ever moving the page under the reader.
  useEffect(() => {
    const list = listRef.current;
    const active = activeRef.current;
    if (!list || !active) return;
    const top = active.offsetTop - list.offsetTop;
    const bottom = top + active.offsetHeight;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (bottom > list.scrollTop + list.clientHeight) {
      list.scrollTop = bottom - list.clientHeight;
    }
  }, [step]);

  // A window rather than the whole list: 509 rows of DOM re-rendered on every
  // step makes playback stutter on the machine this was built on.
  const from = Math.max(0, step - 12);
  const to = Math.min(timeline.length, step + 13);
  const window = timeline.slice(from, to);

  return (
    <ol className="event-log" aria-label="Captured events" ref={listRef}>
      {window.map((event, i) => {
        const index = from + i;
        const active = index === step;
        return (
          <li
            key={event.seq}
            ref={active ? activeRef : undefined}
            className={`log-row ${active ? "log-active" : ""} ${
              index > step ? "log-future" : ""
            }`}
          >
            <button type="button" onClick={() => onSeek(index)}>
              <span className="mono log-time">{timeOf(event.at)}</span>
              <span className={`pill pill-dim log-kind`}>{event.kind}</span>
              <span className="log-summary">{event.summary}</span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}

/** Node and pod counts at the current position, for the strip under the header. */
export function PositionStrip({ state }: { state: ReplayState | null }) {
  if (!state) return null;
  return (
    <div className="position-strip">
      <span>
        <strong>{state.cordoned_nodes}</strong> cordoned
      </span>
      <span className={state.pending_pods > 0 ? "strip-bad" : ""}>
        <strong>{state.pending_pods}</strong> Pending
      </span>
      <span>
        <strong>{state.nodes.length}</strong> nodes
      </span>
      <span>
        <strong>{state.pods.length}</strong> pods
      </span>
      <span className="mono">{timeOf(state.at)}</span>
    </div>
  );
}

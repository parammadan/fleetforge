// The replay data layer.
//
// Two responsibilities, deliberately separate:
//
// - `useReplayBundle` loads the things that do not change — context, chapters,
//   claims, the finding, predictions, traffic, the artifact index. One fetch
//   each, at mount.
// - `usePlayback` owns the position and the clock, and fetches the state for
//   whatever position it is on.
//
// The browser never computes cluster state. It asks the Rust API what the state
// at position N is and renders the answer. That is the whole reason the replay
// engine lives in Rust: a fold implemented twice is a fold that disagrees with
// itself, and the version that disagrees on stage is always the browser's.

import { useCallback, useEffect, useRef, useState } from "react";
import type { Envelope, Mode } from "./types";
import type {
  ArtifactContent,
  ArtifactRef,
  Chapter,
  Claim,
  InvestigationChain,
  PdbArithmetic,
  PredictionRow,
  ReplayContextResponse,
  ReplayEvent,
  ReplayState,
  TrafficValidation,
} from "./replayTypes";

async function getEnvelope<T>(url: string): Promise<Envelope<T>> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url}: HTTP ${response.status}`);
  }
  const envelope = (await response.json()) as Envelope<T>;
  // The backend controls the mode and a test pins it there. This check is the
  // browser's own refusal to render replay data under any other label — if the
  // two ever disagree, the screen must fail loudly rather than pick one.
  if (envelope.mode !== "replay") {
    throw new Error(
      `refusing to render: ${url} returned mode "${envelope.mode}", expected "replay"`,
    );
  }
  return envelope;
}

export interface ReplayBundle {
  context: ReplayContextResponse;
  chapters: Chapter[];
  claims: Claim[];
  chain: InvestigationChain;
  pdb: PdbArithmetic;
  predictions: PredictionRow[];
  traffic: TrafficValidation;
  artifacts: ArtifactRef[];
  timeline: ReplayEvent[];
  mode: Mode;
  modeLabel: string;
}

export type BundleState =
  | { kind: "loading" }
  | { kind: "ready"; bundle: ReplayBundle }
  | { kind: "error"; message: string };

export function useReplayBundle(baseUrl = ""): BundleState {
  const [state, setState] = useState<BundleState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const [
          context,
          chapters,
          claims,
          chain,
          pdb,
          predictions,
          traffic,
          artifacts,
          timeline,
        ] = await Promise.all([
            getEnvelope<ReplayContextResponse>(`${baseUrl}/api/v1/replay/context`),
            getEnvelope<Chapter[]>(`${baseUrl}/api/v1/replay/chapters`),
            getEnvelope<Claim[]>(`${baseUrl}/api/v1/replay/claims`),
            getEnvelope<InvestigationChain>(`${baseUrl}/api/v1/replay/chain`),
            getEnvelope<PdbArithmetic>(`${baseUrl}/api/v1/replay/finding`),
            getEnvelope<PredictionRow[]>(`${baseUrl}/api/v1/replay/predictions`),
            getEnvelope<TrafficValidation>(`${baseUrl}/api/v1/replay/traffic`),
            getEnvelope<ArtifactRef[]>(`${baseUrl}/api/v1/replay/artifacts`),
            getEnvelope<ReplayEvent[]>(`${baseUrl}/api/v1/replay/timeline`),
          ]);
        if (cancelled) return;
        setState({
          kind: "ready",
          bundle: {
            context: context.data,
            chapters: chapters.data,
            claims: claims.data,
            chain: chain.data,
            pdb: pdb.data,
            predictions: predictions.data,
            traffic: traffic.data,
            artifacts: artifacts.data,
            timeline: timeline.data,
            mode: context.mode,
            modeLabel: context.mode_label,
          },
        });
      } catch (e) {
        if (cancelled) return;
        setState({ kind: "error", message: e instanceof Error ? e.message : String(e) });
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [baseUrl]);

  return state;
}

/** Playback speeds, as a multiple of the significant-event step rate. */
export const SPEEDS = [0.5, 1, 2, 4] as const;
export type Speed = (typeof SPEEDS)[number];

/** Milliseconds between steps at 1x. Slow enough to read a line of the log. */
const BASE_STEP_MS = 900;

export interface Playback {
  /** Index into the *significant* timeline, not the raw event index. */
  step: number;
  /** The raw event index the current step maps to. */
  position: number;
  playing: boolean;
  speed: Speed;
  state: ReplayState | null;
  /** True while the first state fetch is outstanding. */
  loading: boolean;
  /**
   * True when the state on screen is for a different position than the one the
   * controls are on.
   *
   * The fold resolves in under a millisecond over loopback, so this is rarely
   * visible — but "rarely" is not "never", and a header reading 14:35 above a
   * fleet drawn at 14:15 is the interface lying about which moment it is
   * showing. Better to say the state is catching up.
   */
  stale: boolean;
  error: string | null;
  play: () => void;
  pause: () => void;
  restart: () => void;
  stepForward: () => void;
  stepBack: () => void;
  seekToStep: (step: number) => void;
  seekToPosition: (position: number) => void;
  setSpeed: (speed: Speed) => void;
}

export function usePlayback(timeline: ReplayEvent[], baseUrl = ""): Playback {
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<Speed>(1);
  const [state, setState] = useState<ReplayState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const lastStep = Math.max(0, timeline.length - 1);
  const clamp = useCallback((n: number) => Math.min(Math.max(n, 0), lastStep), [lastStep]);
  const position = timeline[clamp(step)]?.index ?? 0;

  // Fetch the state for the current position. An in-flight request for an
  // older position must not overwrite a newer one — scrubbing fast makes
  // responses arrive out of order, and the last one to land would win.
  const inFlight = useRef(0);
  useEffect(() => {
    if (timeline.length === 0) return;
    const token = ++inFlight.current;
    let cancelled = false;
    const load = async () => {
      try {
        const envelope = await getEnvelope<ReplayState>(
          `${baseUrl}/api/v1/replay/state?position=${position}`,
        );
        if (cancelled || token !== inFlight.current) return;
        setState(envelope.data);
        setError(null);
      } catch (e) {
        if (cancelled || token !== inFlight.current) return;
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled && token === inFlight.current) setLoading(false);
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [position, baseUrl, timeline.length]);

  // The clock. Stops itself at the end rather than looping: a replay that
  // silently restarts makes a viewer think they are seeing a second incident.
  useEffect(() => {
    if (!playing) return undefined;
    if (step >= lastStep) {
      setPlaying(false);
      return undefined;
    }
    const id = window.setTimeout(() => {
      setStep((s) => clamp(s + 1));
    }, BASE_STEP_MS / speed);
    return () => window.clearTimeout(id);
  }, [playing, step, speed, lastStep, clamp]);

  const seekToPosition = useCallback(
    (target: number) => {
      // Map a raw event index onto the nearest significant step at or before
      // it, so a chapter jump lands on a step the scrubber can represent.
      let best = 0;
      for (let i = 0; i < timeline.length; i += 1) {
        const entry = timeline[i];
        if (entry && entry.index <= target) best = i;
        else break;
      }
      setStep(best);
    },
    [timeline],
  );

  return {
    step: clamp(step),
    position,
    playing,
    speed,
    state,
    loading,
    stale: state !== null && state.position !== position,
    error,
    play: useCallback(() => setPlaying(true), []),
    pause: useCallback(() => setPlaying(false), []),
    restart: useCallback(() => {
      setPlaying(false);
      setStep(0);
    }, []),
    stepForward: useCallback(() => {
      setPlaying(false);
      setStep((s) => clamp(s + 1));
    }, [clamp]),
    stepBack: useCallback(() => {
      setPlaying(false);
      setStep((s) => clamp(s - 1));
    }, [clamp]),
    seekToStep: useCallback(
      (s: number) => {
        setPlaying(false);
        setStep(clamp(s));
      },
      [clamp],
    ),
    seekToPosition: useCallback(
      (p: number) => {
        setPlaying(false);
        seekToPosition(p);
      },
      [seekToPosition],
    ),
    setSpeed,
  };
}

/** Fetch one artifact's redacted content on demand. */
export async function fetchArtifact(name: string, baseUrl = ""): Promise<ArtifactContent> {
  const envelope = await getEnvelope<ArtifactContent>(
    `${baseUrl}/api/v1/replay/artifacts/${encodeURIComponent(name)}`,
  );
  return envelope.data;
}

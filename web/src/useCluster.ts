// The single source of cluster state for the interface.
//
// Everything arrives over Server-Sent Events. There is no polling timer
// anywhere in this file, and that is deliberate: a timer that re-fetches would
// make a dead stream look identical to a quiet cluster.
//
// Instead the backend sends a heartbeat every 10s. If one does not arrive in
// time, the stream is reported *stale* — the data on screen is still shown, and
// it is labelled as no longer current.

import { useCallback, useEffect, useRef, useState } from "react";
import type { ClusterSnapshot, Envelope, Environment, StreamState } from "./types";

/** Heartbeats arrive every 10s; allow two to be missed before declaring stale. */
const STALE_AFTER_MS = 25_000;
const STALE_CHECK_INTERVAL_MS = 2_000;

export interface ClusterState {
  snapshot: ClusterSnapshot | null;
  environment: Environment | null;
  stream: StreamState;
  /** True only when every watched kind was collected authoritatively. */
  authoritative: boolean;
  /** A transport or API error, distinct from a cluster-side problem. */
  error: string | null;
  /** How many distinct snapshots have been received this session. */
  updateCount: number;
}

export function useCluster(baseUrl = ""): ClusterState {
  const [snapshot, setSnapshot] = useState<ClusterSnapshot | null>(null);
  const [environment, setEnvironment] = useState<Environment | null>(null);
  const [stream, setStream] = useState<StreamState>({ kind: "connecting" });
  const [authoritative, setAuthoritative] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [updateCount, setUpdateCount] = useState(0);
  const lastEventAt = useRef<number>(Date.now());

  const refreshEnvironment = useCallback(async () => {
    try {
      const response = await fetch(`${baseUrl}/api/v1/environment`);
      if (!response.ok) throw new Error(`environment: HTTP ${response.status}`);
      setEnvironment((await response.json()) as Environment);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [baseUrl]);

  useEffect(() => {
    void refreshEnvironment();

    const source = new EventSource(`${baseUrl}/api/v1/stream`);

    const markAlive = () => {
      lastEventAt.current = Date.now();
      setStream({ kind: "open", lastEventAt: lastEventAt.current });
      setError(null);
    };

    source.addEventListener("snapshot.updated", (event) => {
      markAlive();
      try {
        const envelope = JSON.parse((event as MessageEvent).data) as Envelope<ClusterSnapshot>;
        setSnapshot(envelope.data);
        setAuthoritative(envelope.authoritative);
        setUpdateCount((n) => n + 1);
      } catch {
        setError("received a malformed snapshot");
      }
    });

    source.addEventListener("heartbeat", (event) => {
      markAlive();
      try {
        const beat = JSON.parse((event as MessageEvent).data) as { authoritative: boolean };
        setAuthoritative(beat.authoritative);
      } catch {
        /* a malformed heartbeat still proves the stream is alive */
      }
      // Coverage can change without the cluster changing — a token expiring,
      // for instance — so re-read the environment on each beat.
      void refreshEnvironment();
    });

    source.addEventListener("stream.lagged", () => {
      markAlive();
      setError("this browser fell behind the stream; showing the latest state");
    });

    source.onerror = () => {
      // EventSource reconnects on its own with Last-Event-ID. Report the gap
      // rather than hiding it behind the retry.
      setStream({ kind: "disconnected", since: Date.now() });
    };

    source.onopen = markAlive;

    // Staleness is the passage of time, not an event. Nothing arrives to tell
    // you the data got old, so it has to be checked.
    const staleCheck = window.setInterval(() => {
      const since = Date.now() - lastEventAt.current;
      setStream((current) => {
        if (current.kind === "disconnected") return current;
        return since > STALE_AFTER_MS
          ? { kind: "stale", lastEventAt: lastEventAt.current }
          : { kind: "open", lastEventAt: lastEventAt.current };
      });
    }, STALE_CHECK_INTERVAL_MS);

    return () => {
      source.close();
      window.clearInterval(staleCheck);
    };
  }, [baseUrl, refreshEnvironment]);

  return { snapshot, environment, stream, authoritative, error, updateCount };
}

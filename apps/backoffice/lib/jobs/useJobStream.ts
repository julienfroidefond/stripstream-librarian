"use client";

import { useCallback, useRef, useState } from "react";

import { isTerminalJobStatus } from "./status";
import { useEventSource } from "../hooks/useEventSource";

/** Statistics reported by the indexer on a job update. */
export interface JobStreamStats {
  scanned_files: number;
  indexed_files: number;
  removed_files: number;
  errors: number;
}

/**
 * Payload pushed on `/api/jobs/:id/stream`. It is the full job detail, but the
 * stream consumers only rely on these fields.
 */
export interface JobStreamEvent {
  id: string;
  status: string;
  current_file: string | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  stats_json: JobStreamStats | null;
}

export type JobStreamError = "connection" | "parse";

interface UseJobStreamOptions {
  /** When false the stream stays closed (e.g. the job is already terminal). */
  enabled?: boolean;
  /** Called for every parsed update. */
  onMessage?: (event: JobStreamEvent) => void;
  /** Called once, when the first terminal status arrives. */
  onComplete?: (event: JobStreamEvent) => void;
}

/**
 * Live updates for one job. Built on `useEventSource`, which owns the
 * transport; this hook only maps messages to state and closes the stream once
 * the job reaches a terminal status.
 */
export function useJobStream(jobId: string, options: UseJobStreamOptions = {}) {
  const { enabled = true, onMessage, onComplete } = options;
  const [job, setJob] = useState<JobStreamEvent | null>(null);
  const [isComplete, setIsComplete] = useState(false);
  const [error, setError] = useState<JobStreamError | null>(null);
  const completedRef = useRef(false);

  const handleMessage = useCallback(
    (event: JobStreamEvent) => {
      setJob(event);
      setError(null);
      onMessage?.(event);

      if (!completedRef.current && isTerminalJobStatus(event.status)) {
        completedRef.current = true;
        setIsComplete(true);
        onComplete?.(event);
      }
    },
    [onMessage, onComplete]
  );

  useEventSource<JobStreamEvent>({
    url: `/api/jobs/${jobId}/stream`,
    onMessage: handleMessage,
    onError: () => setError("connection"),
    onParseError: () => setError("parse"),
    enabled: enabled && !isComplete,
  });

  return { job, isComplete, error };
}

interface UseJobsStreamOptions<TJob> {
  /** Jobs rendered before the first stream update (server-rendered list). */
  initial: TJob[];
  enabled?: boolean;
}

/**
 * Live updates for the whole jobs list (`/api/jobs/stream`). The stream sends
 * the full list whenever it changes.
 */
export function useJobsStream<TJob>({ initial, enabled = true }: UseJobsStreamOptions<TJob>) {
  const [jobs, setJobs] = useState<TJob[]>(initial);

  useEventSource<unknown>({
    url: "/api/jobs/stream",
    onMessage: (data) => {
      if (Array.isArray(data)) setJobs(data as TJob[]);
    },
    enabled,
  });

  return { jobs, setJobs };
}

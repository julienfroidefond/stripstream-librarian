"use client";

import { useEffect, useRef } from "react";

interface UseEventSourceOptions<T> {
  url: string;
  onMessage: (data: T) => void;
  /** Called when the connection drops (before the automatic reconnect). */
  onError?: () => void;
  /** Called when a payload cannot be parsed as JSON. */
  onParseError?: (error: unknown) => void;
  /** When false, no connection is opened (and an open one is closed). */
  enabled?: boolean;
  staleTimeoutMs?: number;
  reconnectDelayMs?: number;
}

/**
 * The only place allowed to open an `EventSource`.
 *
 * Owns reconnection, the stale-connection watchdog and the tab-visibility
 * pause/resume. Consumers describe *what* to do with messages; they never see
 * the transport (see `useJobStream` for the job-specific wrappers).
 */
export function useEventSource<T>({
  url,
  onMessage,
  onError,
  onParseError,
  enabled = true,
  staleTimeoutMs = 30000,
  reconnectDelayMs = 3000,
}: UseEventSourceOptions<T>) {
  const onMessageRef = useRef(onMessage);
  const onErrorRef = useRef(onError);
  const onParseErrorRef = useRef(onParseError);
  const urlRef = useRef(url);

  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    onErrorRef.current = onError;
  }, [onError]);

  useEffect(() => {
    onParseErrorRef.current = onParseError;
  }, [onParseError]);

  useEffect(() => {
    urlRef.current = url;
  }, [url]);

  useEffect(() => {
    if (!enabled) return;

    let eventSource: EventSource | null = null;
    let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    let staleTimeout: ReturnType<typeof setTimeout> | null = null;

    const resetStaleTimer = () => {
      if (staleTimeout) clearTimeout(staleTimeout);
      staleTimeout = setTimeout(() => {
        eventSource?.close();
        eventSource = null;
        connect();
      }, staleTimeoutMs);
    };

    const connect = () => {
      if (eventSource) {
        eventSource.close();
      }
      eventSource = new EventSource(urlRef.current);
      resetStaleTimer();

      eventSource.onmessage = (event) => {
        resetStaleTimer();
        try {
          onMessageRef.current(JSON.parse(event.data));
        } catch (error) {
          onParseErrorRef.current?.(error);
        }
      };

      eventSource.onerror = () => {
        eventSource?.close();
        eventSource = null;
        onErrorRef.current?.();
        reconnectTimeout = setTimeout(connect, reconnectDelayMs);
      };
    };

    const disconnect = () => {
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
        reconnectTimeout = null;
      }
      if (staleTimeout) {
        clearTimeout(staleTimeout);
        staleTimeout = null;
      }
      if (eventSource) {
        eventSource.close();
        eventSource = null;
      }
    };

    const handleVisibilityChange = () => {
      if (document.hidden) {
        disconnect();
      } else {
        connect();
      }
    };

    connect();
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      disconnect();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [staleTimeoutMs, reconnectDelayMs, enabled]);
}

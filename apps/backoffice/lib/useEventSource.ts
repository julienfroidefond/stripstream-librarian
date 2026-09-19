"use client";

import { useEffect, useRef } from "react";

interface UseEventSourceOptions<T> {
  url: string;
  onMessage: (data: T) => void;
  staleTimeoutMs?: number;
  reconnectDelayMs?: number;
}

export function useEventSource<T>({
  url,
  onMessage,
  staleTimeoutMs = 30000,
  reconnectDelayMs = 3000,
}: UseEventSourceOptions<T>) {
  const onMessageRef = useRef(onMessage);
  const urlRef = useRef(url);

  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    urlRef.current = url;
  }, [url]);

  useEffect(() => {
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
        } catch {
          // ignore malformed data
        }
      };

      eventSource.onerror = () => {
        eventSource?.close();
        eventSource = null;
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
  }, [staleTimeoutMs, reconnectDelayMs]);
}

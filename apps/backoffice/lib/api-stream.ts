import { NextRequest } from "next/server";

import { config } from "@/lib/api";

interface PollingStreamOptions {
  /** Path on the Rust API, e.g. `/torrent-downloads`. */
  path: string;
  /**
   * Whether the fetched list still contains at least one moving item. Drives the
   * polling cadence: fast while active, slow when idle.
   */
  hasActive: (data: Array<{ status: string }>) => boolean;
  /** Label used in the fetch-error warning log. */
  label: string;
  /** Poll interval while active items exist (default 2s). */
  activeIntervalMs?: number;
  /** Poll interval while idle (default 10s). */
  idleIntervalMs?: number;
  /** Heartbeat interval keeping the connection alive (default 15s). */
  heartbeatMs?: number;
}

/**
 * Shared implementation for the SSE routes mirroring a Rust API collection
 * (`/index/status`, `/torrent-downloads`, `/telegram-monitor/downloads`, ...).
 *
 * Emits the whole list on connect, re-polls at a cadence that follows activity,
 * and sends a heartbeat so the connection survives idle periods.
 */
export function pollingStream(request: NextRequest, options: PollingStreamOptions): Response {
  const { baseUrl, token } = config();
  const {
    path,
    hasActive,
    label,
    activeIntervalMs = 2000,
    idleIntervalMs = 10000,
    heartbeatMs = 15000,
  } = options;

  const encoder = new TextEncoder();

  const stream = new ReadableStream({
    async start(controller) {
      controller.enqueue(encoder.encode(""));

      let isActive = true;
      let consecutiveErrors = 0;
      let intervalId: ReturnType<typeof setInterval> | null = null;
      let heartbeatId: ReturnType<typeof setInterval> | null = null;

      const send = (msg: string): boolean => {
        try {
          controller.enqueue(encoder.encode(msg));
          return true;
        } catch {
          isActive = false;
          return false;
        }
      };

      function restartInterval(ms: number) {
        if (intervalId !== null) clearInterval(intervalId);
        intervalId = setInterval(fetchData, ms);
      }

      async function fetchData() {
        if (!isActive) return;

        try {
          const response = await fetch(`${baseUrl}${path}`, {
            headers: { Authorization: `Bearer ${token}` },
          });

          if (response.ok && isActive) {
            consecutiveErrors = 0;
            const data = await response.json();

            if (isActive) {
              send(`data: ${JSON.stringify(data)}\n\n`);
            }

            restartInterval(hasActive(data) ? activeIntervalMs : idleIntervalMs);
          }
        } catch (error) {
          if (isActive) {
            consecutiveErrors++;
            if (consecutiveErrors === 1 || consecutiveErrors % 30 === 0) {
              console.warn(`SSE ${label} fetch error (${consecutiveErrors} consecutive):`, error);
            }
          }
        }
      }

      // Heartbeat every 15s to keep connection alive
      heartbeatId = setInterval(() => {
        if (isActive) send(": heartbeat\n\n");
      }, heartbeatMs);

      // Initial fetch + start polling
      await fetchData();

      // Cleanup
      request.signal.addEventListener("abort", () => {
        isActive = false;
        if (intervalId !== null) clearInterval(intervalId);
        if (heartbeatId !== null) clearInterval(heartbeatId);
        controller.close();
      });
    },
  });

  return new Response(stream, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      "Connection": "keep-alive",
    },
  });
}

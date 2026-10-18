import { NextRequest } from "next/server";

import { config } from "@/lib/api";

/** A Rust API collection folded into a combined stream payload. */
export interface StreamSource {
  /** Key the fetched list is emitted under. */
  key: string;
  /** Path on the Rust API, e.g. `/torrent-downloads`. */
  path: string;
}

interface PollResult {
  /** Payload emitted as the SSE `data:` frame. */
  payload: unknown;
  /** Whether the payload still contains at least one moving item. */
  active: boolean;
}

interface StreamLoopOptions extends PollingStreamsBase {
  /** Fetches and folds the payload to emit. */
  poll: () => Promise<PollResult>;
}

interface PollingStreamsBase {
  /** Label used in the fetch-error warning log. */
  label: string;
  /** Poll interval while active items exist (default 2s). */
  activeIntervalMs?: number;
  /** Poll interval while idle (default 10s). */
  idleIntervalMs?: number;
  /** Heartbeat interval keeping the connection alive (default 15s). */
  heartbeatMs?: number;
}

interface PollingStreamOptions extends PollingStreamsBase {
  /** Path on the Rust API, e.g. `/torrent-downloads`. */
  path: string;
  /**
   * Whether the fetched list still contains at least one moving item. Drives the
   * polling cadence: fast while active, slow when idle.
   */
  hasActive: (data: Array<{ status: string }>) => boolean;
}

interface PollingStreamsOptions extends PollingStreamsBase {
  /** API collections folded into a `{ key: list }` payload. */
  sources: StreamSource[];
  /** Cadence driver: true while at least one collection still moves. */
  hasActive: (data: Record<string, Array<{ status: string }>>) => boolean;
}

/**
 * Shared implementation for the SSE routes mirroring a Rust API collection
 * (`/index/status`, `/torrent-downloads`, ...).
 *
 * Emits the whole list on connect, re-polls at a cadence that follows activity,
 * and sends a heartbeat so the connection survives idle periods.
 */
export function pollingStream(request: NextRequest, options: PollingStreamOptions): Response {
  const { baseUrl, token } = config();

  return streamLoop(request, {
    label: options.label,
    activeIntervalMs: options.activeIntervalMs,
    idleIntervalMs: options.idleIntervalMs,
    heartbeatMs: options.heartbeatMs,
    poll: async () => {
      const data = await fetchList(baseUrl, token, options.path);
      return { payload: data, active: options.hasActive(data) };
    },
  });
}

/**
 * Same contract as {@link pollingStream}, but folds several Rust API collections
 * into **one** connection, emitted as a `{ key: list }` object.
 *
 * Browsers cap concurrent connections per origin (6 over HTTP/1.1) and every SSE
 * stream holds one for the whole session. One `EventSource` per collection
 * (torrents + Telegram + DDL, plus the jobs one) sits right at that cap, which
 * starves the App Router navigation fetches: clicking a link leaves the page
 * stuck on "rendering" until a stream closes. Prefer this over one route per
 * collection when a screen watches several.
 */
export function pollingStreams(request: NextRequest, options: PollingStreamsOptions): Response {
  const { baseUrl, token } = config();

  return streamLoop(request, {
    label: options.label,
    activeIntervalMs: options.activeIntervalMs,
    idleIntervalMs: options.idleIntervalMs,
    heartbeatMs: options.heartbeatMs,
    poll: async () => {
      const lists = await Promise.all(
        options.sources.map((source) => fetchList(baseUrl, token, source.path))
      );
      const data: Record<string, Array<{ status: string }>> = {};
      options.sources.forEach((source, index) => {
        data[source.key] = lists[index];
      });
      return { payload: data, active: options.hasActive(data) };
    },
  });
}

async function fetchList(
  baseUrl: string,
  token: string,
  path: string
): Promise<Array<{ status: string }>> {
  const response = await fetch(`${baseUrl}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok) {
    throw new Error(`${path} responded with ${response.status}`);
  }
  return response.json();
}

function streamLoop(request: NextRequest, options: StreamLoopOptions): Response {
  const {
    poll,
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
          const { payload, active } = await poll();

          if (isActive) {
            consecutiveErrors = 0;
            send(`data: ${JSON.stringify(payload)}\n\n`);
            restartInterval(active ? activeIntervalMs : idleIntervalMs);
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

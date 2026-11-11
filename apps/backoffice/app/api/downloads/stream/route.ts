import { NextRequest } from "next/server";

import { pollingStreams } from "@/lib/api-stream";
import { hasActiveDownloads } from "@/lib/downloads/stream";

/**
 * Every download source over a single SSE connection.
 *
 * One stream per source would saturate the browser's per-origin connection cap,
 * starving the App Router navigation fetches (clicking a link stays on
 * "rendering" forever). Fold the three lists into one payload instead.
 */
export async function GET(request: NextRequest) {
  return pollingStreams(request, {
    label: "downloads",
    activeIntervalMs: 1000,
    sources: [
      { key: "torrent", path: "/torrent-downloads" },
      { key: "telegram", path: "/telegram-monitor/downloads" },
      { key: "ddl", path: "/ddl-provider-downloads" },
    ],
    hasActive: hasActiveDownloads,
  });
}

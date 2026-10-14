import { NextRequest } from "next/server";

import { pollingStream } from "@/lib/api-stream";

const ACTIVE_STATUSES = new Set(["downloading", "completed", "importing"]);

export async function GET(request: NextRequest) {
  return pollingStream(request, {
    path: "/torrent-downloads",
    label: "torrent-downloads",
    hasActive: (downloads) => downloads.some((d) => ACTIVE_STATUSES.has(d.status)),
  });
}

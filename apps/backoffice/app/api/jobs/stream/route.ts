import { NextRequest } from "next/server";

import { pollingStream } from "@/lib/api-stream";

const ACTIVE_STATUSES = new Set([
  "running",
  "pending",
  "extracting_pages",
  "generating_thumbnails",
]);

export async function GET(request: NextRequest) {
  return pollingStream(request, {
    path: "/index/status",
    label: "jobs",
    hasActive: (jobs) => jobs.some((job) => ACTIVE_STATUSES.has(job.status)),
  });
}

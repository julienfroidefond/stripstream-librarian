import { NextRequest } from "next/server";

import { pollingStream } from "@/lib/api-stream";

export async function GET(request: NextRequest) {
  return pollingStream(request, {
    path: "/telegram-monitor/downloads",
    label: "telegram-downloads",
    hasActive: (downloads) => downloads.some((d) => d.status === "downloading"),
  });
}

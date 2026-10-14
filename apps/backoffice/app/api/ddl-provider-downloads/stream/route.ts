import { NextRequest } from "next/server";

import { pollingStream } from "@/lib/api-stream";

export async function GET(request: NextRequest) {
  return pollingStream(request, {
    path: "/ddl-provider-downloads",
    label: "ddl-provider-downloads",
    hasActive: (downloads) => downloads.some((d) => d.status === "queued" || d.status === "downloading"),
  });
}

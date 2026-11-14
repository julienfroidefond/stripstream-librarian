import { NextRequest, NextResponse } from "next/server";
import { markSeriesRead } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await markSeriesRead(body.series, body.status ?? "read");
  invalidateAfter({
    type: "readingProgress.changed",
    seriesId: body.series && body.series !== "unclassified" ? body.series : null,
  });
  return NextResponse.json(data);
}, { fallback: "Failed to mark series" });

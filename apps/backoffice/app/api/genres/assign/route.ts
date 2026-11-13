import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/genres/assign", {
    method: "POST",
    body: JSON.stringify(body),
  });
  invalidateAfter({
    type: "genres.changed",
    seriesIds: Array.isArray(body?.series_ids) ? body.series_ids : undefined,
  });
  return NextResponse.json(data);
}, { fallback: "Failed" });

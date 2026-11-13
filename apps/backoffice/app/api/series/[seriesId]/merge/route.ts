import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

type Params = Promise<{ seriesId: string }>;

export const POST = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const body = await request.json();
  const data = await apiFetch(`/series/${seriesId}/merge`, {
    method: "POST",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "series.merged", targetId: seriesId, sourceId: body?.source_id });
  return NextResponse.json(data);
}, { fallback: "Failed to merge series" });

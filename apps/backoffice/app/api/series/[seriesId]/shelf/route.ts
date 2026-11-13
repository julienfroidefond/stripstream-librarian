import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

type Params = Promise<{ seriesId: string }>;

export const PUT = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  await apiFetch(`/series/${seriesId}/shelf`, { method: "PUT" });
  invalidateAfter({ type: "series.updated", seriesId });
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed to set series aside" });

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  await apiFetch(`/series/${seriesId}/shelf`, { method: "DELETE" });
  invalidateAfter({ type: "series.updated", seriesId });
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed to resume series" });

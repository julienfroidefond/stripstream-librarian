import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

type Params = Promise<{ seriesId: string }>;

export const PATCH = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const body = await request.json();
  const data = await apiFetch(`/series/${seriesId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "series.updated", seriesId });
  return NextResponse.json(data);
}, { fallback: "Failed to update series" });

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const data = await apiFetch(`/series/${seriesId}`, { method: "DELETE" });
  invalidateAfter({ type: "series.deleted", seriesId });
  return NextResponse.json(data);
}, { fallback: "Failed to delete series" });

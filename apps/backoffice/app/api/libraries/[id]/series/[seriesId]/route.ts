import { NextRequest, NextResponse } from "next/server";
import { updateSeries, deleteSeries } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ id: string; seriesId: string }> }) => {
  const { seriesId } = await params;
  const body = await request.json();
  const data = await updateSeries(seriesId, body);
  invalidateAfter({ type: "series.updated", seriesId });
  return NextResponse.json(data);
}, { fallback: "Failed to update series" });

export const DELETE = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string; seriesId: string }> }) => {
  const { seriesId } = await params;
  await deleteSeries(seriesId);
  invalidateAfter({ type: "series.deleted", seriesId });
  return NextResponse.json({ deleted: true });
}, { fallback: "Failed to delete series" });

import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

type Params = Promise<{ seriesId: string }>;

export const GET = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const data = await apiFetch(`/series/${seriesId}/anilist`);
  return NextResponse.json(data);
}, { fallback: "Not found", status: 404 });

export const POST = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const body = await request.json();
  const data = await apiFetch(`/series/${seriesId}/anilist/link`, {
    method: "POST",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "series.updated", seriesId });
  return NextResponse.json(data);
}, { fallback: "Failed to link series" });

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const data = await apiFetch(`/series/${seriesId}/anilist/unlink`, { method: "DELETE" });
  invalidateAfter({ type: "series.updated", seriesId });
  return NextResponse.json(data);
}, { fallback: "Failed to unlink series" });

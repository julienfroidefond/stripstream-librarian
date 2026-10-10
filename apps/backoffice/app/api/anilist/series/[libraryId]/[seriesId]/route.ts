import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ libraryId: string; seriesId: string }>;

export const GET = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { libraryId, seriesId } = await params;
  const data = await apiFetch(
    `/anilist/series/${libraryId}/${seriesId}`,
  );
  return NextResponse.json(data);
}, { fallback: "Not found", status: 404 });

export const POST = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { libraryId, seriesId } = await params;
  const body = await request.json();
  const data = await apiFetch(
    `/anilist/series/${libraryId}/${seriesId}/link`,
    { method: "POST", body: JSON.stringify(body) },
  );
  return NextResponse.json(data);
}, { fallback: "Failed to link series" });

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { libraryId, seriesId } = await params;
  const data = await apiFetch(
    `/anilist/series/${libraryId}/${seriesId}/unlink`,
    { method: "DELETE" },
  );
  return NextResponse.json(data);
}, { fallback: "Failed to unlink series" });

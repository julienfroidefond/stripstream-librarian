import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },) => {
  const { id } = await params;
  const body = await request.json();
  const data = await apiFetch(`/anilist/libraries/${id}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  return NextResponse.json(data);
}, { fallback: "Failed to update library AniList setting" });

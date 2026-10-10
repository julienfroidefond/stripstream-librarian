import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },) => {
  const { id } = await params;
  const data = await apiFetch(`/torrent-downloads/${id}/retry`, { method: "POST" });
  return NextResponse.json(data);
}, { fallback: "Failed to retry import" });

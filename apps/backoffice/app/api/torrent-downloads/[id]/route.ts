import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const data = await apiFetch(`/torrent-downloads/${id}`, { method: "DELETE" });
  return NextResponse.json(data);
}, { fallback: "Failed to delete torrent download" });

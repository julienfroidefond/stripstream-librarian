import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const qs = request.nextUrl.search;
  const data = await apiFetch(`/available-downloads/${id}${qs}`, { method: "DELETE" });
  return NextResponse.json(data);
}, { fallback: "Failed to delete available download" });

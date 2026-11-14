import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const PATCH = withRoute(async (
  request: Request,
  { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const body = await request.json();
  const data = await apiFetch(`/libraries/${id}/reading-status-provider`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "library.updated", libraryId: id });
  return NextResponse.json(data);
}, { fallback: "Failed to update reading status provider" });

import { NextRequest, NextResponse } from "next/server";
import { apiFetch, LibraryDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const body = await request.json();
  const data = await apiFetch<LibraryDto>(`/libraries/${id}/metadata-provider`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "library.updated", libraryId: id });
  return NextResponse.json(data);
}, { fallback: "Failed to update metadata provider" });

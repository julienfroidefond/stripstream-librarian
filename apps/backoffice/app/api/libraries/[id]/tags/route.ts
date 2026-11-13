import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

type Params = Promise<{ id: string }>;

export const PATCH = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { id } = await params;
  const body = await request.json();
  const data = await apiFetch(`/libraries/${id}/tags`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "library.updated", libraryId: id });
  return NextResponse.json(data);
}, { fallback: "Failed to update tags" });

import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { ApiError } from "@/lib/api/errors";
import { withRoute } from "@/lib/server/api-handler";

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const qs = request.nextUrl.search;
  try {
    const data = await apiFetch(`/available-downloads/${id}${qs}`, { method: "DELETE" });
    return NextResponse.json(data);
  } catch (error) {
    // Forward client errors (notably the 409 "release changed" of an
    // index-based removal) so the page can tell them from a server failure.
    if (error instanceof ApiError && error.status >= 400 && error.status < 500) {
      return NextResponse.json({ error: error.detail ?? error.message }, { status: error.status });
    }
    throw error;
  }
}, { fallback: "Failed to delete available download" });

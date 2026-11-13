import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch<{ status: string }>(`/metadata/reject/${body.id}`, {
    method: "POST",
  });
  invalidateAfter({ type: "metadata.changed" });
  return NextResponse.json(data);
}, { fallback: "Failed to reject metadata" });

import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const { id, ...rest } = body;
  const data = await apiFetch<{ status: string; books_synced: number }>(`/metadata/approve/${id}`, {
    method: "POST",
    body: JSON.stringify(rest),
  });
  invalidateAfter({ type: "metadata.changed" });
  return NextResponse.json(data);
}, { fallback: "Failed to approve metadata" });

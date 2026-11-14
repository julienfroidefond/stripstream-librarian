import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (_request: NextRequest, { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const data = await apiFetch(`/metadata/refresh-link/${id}`, { method: "POST" });
  invalidateAfter({ type: "metadata.changed", linkId: id });
  return NextResponse.json(data);
}, { fallback: "Failed to refresh metadata" });

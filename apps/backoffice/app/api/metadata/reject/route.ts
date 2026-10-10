import { NextRequest, NextResponse } from "next/server";
import { revalidateTag } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch<{ status: string }>(`/metadata/reject/${body.id}`, {
    method: "POST",
  });
  revalidateTag("metadata", { expire: 0 });
  return NextResponse.json(data);
}, { fallback: "Failed to reject metadata" });

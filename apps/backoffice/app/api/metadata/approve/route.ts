import { NextRequest, NextResponse } from "next/server";
import { revalidateTag } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const { id, ...rest } = body;
  const data = await apiFetch<{ status: string; books_synced: number }>(`/metadata/approve/${id}`, {
    method: "POST",
    body: JSON.stringify(rest),
  });
  revalidateTag("metadata", { expire: 0 });
  return NextResponse.json(data);
}, { fallback: "Failed to approve metadata" });

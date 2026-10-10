import { NextRequest, NextResponse } from "next/server";
import { revalidateTag } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (_request: NextRequest, { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const data = await apiFetch(`/metadata/refresh-link/${id}`, { method: "POST" });
  revalidateTag("metadata", { expire: 0 });
  return NextResponse.json(data);
}, { fallback: "Failed to refresh metadata" });

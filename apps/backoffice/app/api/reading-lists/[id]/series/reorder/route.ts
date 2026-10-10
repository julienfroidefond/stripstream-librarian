import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ id: string }>;

export const PUT = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { id } = await params;
  const body = await request.json();
  await apiFetch(`/reading-lists/${id}/series/reorder`, {
    method: "PUT",
    body: JSON.stringify(body),
  });
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed" });

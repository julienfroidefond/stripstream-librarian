import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ id: string; seriesId: string }>;

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { id, seriesId } = await params;
  await apiFetch(`/reading-lists/${id}/series/${seriesId}`, { method: "DELETE" });
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed" });

import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

type Params = Promise<{ name: string }>;

export const PATCH = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { name } = await params;
  const body = await request.json();
  const data = await apiFetch(`/genres/${encodeURIComponent(name)}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "genres.changed" });
  return NextResponse.json(data);
}, { fallback: "Failed" });

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { name } = await params;
  const data = await apiFetch(`/genres/${encodeURIComponent(name)}`, {
    method: "DELETE",
  });
  invalidateAfter({ type: "genres.changed" });
  return NextResponse.json(data);
}, { fallback: "Failed" });

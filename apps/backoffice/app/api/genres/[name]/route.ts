import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ name: string }>;

export const PATCH = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { name } = await params;
  const body = await request.json();
  const data = await apiFetch(`/genres/${encodeURIComponent(name)}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  return NextResponse.json(data);
}, { fallback: "Failed" });

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { name } = await params;
  const data = await apiFetch(`/genres/${encodeURIComponent(name)}`, {
    method: "DELETE",
  });
  return NextResponse.json(data);
}, { fallback: "Failed" });

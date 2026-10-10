import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },) => {
  const { id } = await params;
  const data = await apiFetch<unknown>(`/admin/series/archived/${id}`);
  return NextResponse.json(data);
}, { fallback: "Failed" });

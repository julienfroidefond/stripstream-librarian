import { NextRequest, NextResponse } from "next/server";
import { apiFetch, MetadataBatchResultDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const { searchParams } = new URL(request.url);
  const id = searchParams.get("id");
  if (!id) {
    return NextResponse.json({ error: "id is required" }, { status: 400 });
  }
  const status = searchParams.get("status") || "";
  const params = status ? `?status=${status}` : "";
  const data = await apiFetch<MetadataBatchResultDto[]>(`/metadata/batch/${id}/results${params}`);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch results" });

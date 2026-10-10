import { NextRequest, NextResponse } from "next/server";
import { apiFetch, MetadataBatchReportDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const { searchParams } = new URL(request.url);
  const id = searchParams.get("id");
  if (!id) {
    return NextResponse.json({ error: "id is required" }, { status: 400 });
  }
  const data = await apiFetch<MetadataBatchReportDto>(`/metadata/batch/${id}/report`);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch report" });

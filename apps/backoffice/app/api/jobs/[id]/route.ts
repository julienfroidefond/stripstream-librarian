import { NextRequest, NextResponse } from "next/server";
import { apiFetch, IndexJobDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const data = await apiFetch<IndexJobDto>(`/index/jobs/${id}`);
  return NextResponse.json(data);
}, { message: "Failed to fetch job" });

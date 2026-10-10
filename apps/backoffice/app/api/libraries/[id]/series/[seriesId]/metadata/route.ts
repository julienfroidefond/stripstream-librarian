import { NextRequest, NextResponse } from "next/server";
import { fetchSeriesMetadata } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string; seriesId: string }> }) => {
  const { seriesId } = await params;
  const data = await fetchSeriesMetadata(seriesId);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch series metadata" });

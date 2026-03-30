import { NextRequest, NextResponse } from "next/server";
import { fetchSeriesMetadata } from "@/lib/api";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string; seriesId: string }> }
) {
  const { seriesId } = await params;
  try {
    const data = await fetchSeriesMetadata(seriesId);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch series metadata";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

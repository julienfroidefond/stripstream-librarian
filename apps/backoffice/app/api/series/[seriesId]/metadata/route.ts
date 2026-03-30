import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ seriesId: string }>;

export async function GET(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const data = await apiFetch(`/series/${seriesId}/metadata`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch metadata";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

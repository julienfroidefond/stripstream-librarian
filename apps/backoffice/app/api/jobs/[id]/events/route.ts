import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const searchParams = request.nextUrl.searchParams;
    const query = new URLSearchParams();
    if (searchParams.has("level")) query.set("level", searchParams.get("level")!);
    if (searchParams.has("event_type")) query.set("event_type", searchParams.get("event_type")!);
    if (searchParams.has("limit")) query.set("limit", searchParams.get("limit")!);
    const qs = query.toString();
    const url = `/index/jobs/${id}/events${qs ? `?${qs}` : ""}`;
    const data = await apiFetch(url);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch job events";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

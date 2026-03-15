import { NextRequest, NextResponse } from "next/server";
import { markSeriesRead } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await markSeriesRead(body.series, body.status ?? "read");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to mark series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

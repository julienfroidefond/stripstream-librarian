import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ seriesId: string }>;

export async function POST(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const body = await request.json();
    const data = await apiFetch(`/series/${seriesId}/merge`, {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to merge series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

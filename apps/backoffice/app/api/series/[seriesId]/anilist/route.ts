import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ seriesId: string }>;

export async function GET(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const data = await apiFetch(`/series/${seriesId}/anilist`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Not found";
    return NextResponse.json({ error: message }, { status: 404 });
  }
}

export async function POST(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const body = await request.json();
    const data = await apiFetch(`/series/${seriesId}/anilist/link`, {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to link series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const data = await apiFetch(`/series/${seriesId}/anilist/unlink`, { method: "DELETE" });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to unlink series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

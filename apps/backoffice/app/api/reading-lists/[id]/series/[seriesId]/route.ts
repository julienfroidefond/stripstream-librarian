import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ id: string; seriesId: string }>;

export async function DELETE(_request: NextRequest, { params }: { params: Params }) {
  try {
    const { id, seriesId } = await params;
    await apiFetch(`/reading-lists/${id}/series/${seriesId}`, { method: "DELETE" });
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

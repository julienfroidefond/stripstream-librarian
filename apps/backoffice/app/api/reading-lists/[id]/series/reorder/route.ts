import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ id: string }>;

export async function PUT(request: NextRequest, { params }: { params: Params }) {
  try {
    const { id } = await params;
    const body = await request.json();
    await apiFetch(`/reading-lists/${id}/series/reorder`, {
      method: "PUT",
      body: JSON.stringify(body),
    });
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

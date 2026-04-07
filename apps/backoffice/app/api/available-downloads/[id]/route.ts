import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function DELETE(request: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  try {
    const { id } = await params;
    const qs = request.nextUrl.search;
    const data = await apiFetch(`/available-downloads/${id}${qs}`, { method: "DELETE" });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to delete available download";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

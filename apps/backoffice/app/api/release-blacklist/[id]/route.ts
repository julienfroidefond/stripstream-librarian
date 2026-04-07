import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function DELETE(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const data = await apiFetch(`/release-blacklist/${id}`, { method: "DELETE" });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to unblacklist release";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

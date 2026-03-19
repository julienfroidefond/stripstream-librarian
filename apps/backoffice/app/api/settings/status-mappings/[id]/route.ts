import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function DELETE(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const data = await apiFetch<unknown>(`/settings/status-mappings/${id}`, {
      method: "DELETE",
    });
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ error: "Failed to delete status mapping" }, { status: 500 });
  }
}

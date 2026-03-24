import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function PATCH(
  request: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const body = await request.json();
    const data = await apiFetch(`/libraries/${id}/reading-status-provider`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update reading status provider";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

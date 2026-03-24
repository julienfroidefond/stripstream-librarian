import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const body = await request.json();
    const data = await apiFetch(`/anilist/libraries/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update library AniList setting";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

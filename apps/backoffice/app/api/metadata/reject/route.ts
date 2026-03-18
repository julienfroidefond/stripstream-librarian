import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch<{ status: string }>(`/metadata/reject/${body.id}`, {
      method: "POST",
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to reject metadata";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

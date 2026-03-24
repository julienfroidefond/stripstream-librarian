import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch("/anilist/sync/preview");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to preview sync";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

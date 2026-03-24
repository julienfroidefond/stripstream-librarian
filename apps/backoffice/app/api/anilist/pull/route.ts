import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function POST() {
  try {
    const data = await apiFetch("/anilist/pull", { method: "POST", body: "{}" });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to pull from AniList";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

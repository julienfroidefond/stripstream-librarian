import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function POST() {
  try {
    const data = await apiFetch("/anilist/sync", { method: "POST", body: "{}" });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to sync to AniList";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch("/prowlarr/test");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to test Prowlarr connection";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch("/torrent-downloads");
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ error: "Failed to fetch torrent downloads" }, { status: 500 });
  }
}

import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch("/download-detection/latest-found");
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ error: "Failed to fetch latest detection results" }, { status: 500 });
  }
}

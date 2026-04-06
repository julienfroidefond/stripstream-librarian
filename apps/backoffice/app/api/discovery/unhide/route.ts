import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch("/discovery/unhide", { method: "POST", body: JSON.stringify(body) });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to unhide suggestion";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

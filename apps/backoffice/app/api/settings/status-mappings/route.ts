import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch<unknown>("/settings/status-mappings");
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ error: "Failed to fetch status mappings" }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch<unknown>("/settings/status-mappings", {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ error: "Failed to save status mapping" }, { status: 500 });
  }
}

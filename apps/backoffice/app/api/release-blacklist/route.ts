import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch("/release-blacklist");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch blacklist";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch("/release-blacklist", { method: "POST", body: JSON.stringify(body) });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to blacklist release";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const limit = searchParams.get("limit") || "100";
    const data = await apiFetch(`/discovery/prowlarr?limit=${limit}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch Prowlarr discovery";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

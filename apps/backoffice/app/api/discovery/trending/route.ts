import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const provider = searchParams.get("provider") || "anilist";
    const limit = searchParams.get("limit") || "24";
    const offset = searchParams.get("offset") || "0";
    const period = searchParams.get("period") || "";
    const nocache = searchParams.get("nocache") || "";
    const data = await apiFetch(`/discovery/trending?provider=${provider}&limit=${limit}&offset=${offset}${period ? `&period=${period}` : ""}${nocache ? "&nocache=true" : ""}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch trending";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

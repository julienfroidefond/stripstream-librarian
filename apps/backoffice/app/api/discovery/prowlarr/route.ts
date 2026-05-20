import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const limit = searchParams.get("limit") || "100";
    const nocache = searchParams.get("nocache") || "";
    const sort = searchParams.get("sort") || "";
    const indexer = searchParams.get("indexer") || "";
    const data = await apiFetch(`/discovery/prowlarr?limit=${limit}${nocache ? "&nocache=true" : ""}${sort ? `&sort=${sort}` : ""}${indexer ? `&indexer=${encodeURIComponent(indexer)}` : ""}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch Prowlarr discovery";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const nocache = searchParams.get("nocache") || "";
    const sort = searchParams.get("sort") || "";
    const indexer = searchParams.get("indexer") || "";
    const category = searchParams.get("category") || "";
    const params = new URLSearchParams();
    if (nocache) params.set("nocache", "true");
    if (sort) params.set("sort", sort);
    if (indexer) params.set("indexer", indexer);
    if (category) params.set("category", category);
    const data = await apiFetch(`/discovery/prowlarr?${params.toString()}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch Prowlarr discovery";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

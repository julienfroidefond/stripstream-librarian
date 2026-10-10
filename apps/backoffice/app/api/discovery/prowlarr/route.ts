import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
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
}, { fallback: "Failed to fetch Prowlarr discovery" });

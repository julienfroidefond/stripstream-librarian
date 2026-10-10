import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const { searchParams } = new URL(request.url);
  const provider = searchParams.get("provider") || "anilist";
  const limit = searchParams.get("limit") || "24";
  const offset = searchParams.get("offset") || "0";
  const period = searchParams.get("period") || "";
  const nocache = searchParams.get("nocache") || "";
  const data = await apiFetch(`/discovery/trending?provider=${provider}&limit=${limit}&offset=${offset}${period ? `&period=${period}` : ""}${nocache ? "&nocache=true" : ""}`);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch trending" });

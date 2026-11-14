import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import type { GenreBrowsePage } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const data = await apiFetch<GenreBrowsePage>(`/genres/browse${request.nextUrl.search}`);
  return NextResponse.json(data);
}, { fallback: "Failed" });

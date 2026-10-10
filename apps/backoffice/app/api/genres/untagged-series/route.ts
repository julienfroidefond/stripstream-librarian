import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const search = request.nextUrl.search;
  const data = await apiFetch<unknown[]>(`/genres/untagged-series${search}`);
  return NextResponse.json(data);
}, { fallback: "Failed" });

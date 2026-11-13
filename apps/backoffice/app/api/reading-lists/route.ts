import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const GET = withRoute(async (request: NextRequest) => {
  // `series_id` narrows the result to the lists holding that series (the
  // "add to list" modal reads it to mark the lists already containing it).
  const seriesId = request?.nextUrl.searchParams.get("series_id");
  const data = await apiFetch(
    seriesId ? `/reading-lists?series_id=${encodeURIComponent(seriesId)}` : "/reading-lists"
  );
  return NextResponse.json(data);
}, { fallback: "Failed" });

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/reading-lists", {
    method: "POST",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "readingLists.changed" });
  return NextResponse.json(data);
}, { fallback: "Failed" });

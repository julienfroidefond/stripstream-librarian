import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const GET = withRoute(async () => {
  const data = await apiFetch("/reading-lists");
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

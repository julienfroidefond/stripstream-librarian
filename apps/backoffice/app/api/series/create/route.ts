import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/series/create", {
    method: "POST",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "series.created" });
  return NextResponse.json(data);
}, { fallback: "Failed to create series" });

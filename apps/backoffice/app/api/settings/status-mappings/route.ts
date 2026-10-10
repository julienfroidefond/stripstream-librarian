import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch<unknown>("/settings/status-mappings");
  return NextResponse.json(data);
}, { message: "Failed to fetch status mappings" });

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch<unknown>("/settings/status-mappings", {
    method: "POST",
    body: JSON.stringify(body),
  });
  return NextResponse.json(data);
}, { message: "Failed to save status mapping" });

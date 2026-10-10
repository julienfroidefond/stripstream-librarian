import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/genres/assign", {
    method: "POST",
    body: JSON.stringify(body),
  });
  return NextResponse.json(data);
}, { fallback: "Failed" });

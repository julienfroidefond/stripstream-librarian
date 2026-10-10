import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch("/release-blacklist");
  return NextResponse.json(data);
}, { fallback: "Failed to fetch blacklist" });

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/release-blacklist", { method: "POST", body: JSON.stringify(body) });
  return NextResponse.json(data);
}, { fallback: "Failed to blacklist release" });

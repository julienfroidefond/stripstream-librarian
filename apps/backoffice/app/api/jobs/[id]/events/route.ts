import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const searchParams = request.nextUrl.searchParams;
  const query = new URLSearchParams();
  if (searchParams.has("level")) query.set("level", searchParams.get("level")!);
  if (searchParams.has("event_type")) query.set("event_type", searchParams.get("event_type")!);
  if (searchParams.has("limit")) query.set("limit", searchParams.get("limit")!);
  const qs = query.toString();
  const url = `/index/jobs/${id}/events${qs ? `?${qs}` : ""}`;
  const data = await apiFetch(url);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch job events" });

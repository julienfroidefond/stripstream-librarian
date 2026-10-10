import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ seriesId: string }>;

export const GET = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const data = await apiFetch(`/series/${seriesId}/metadata`);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch metadata" });

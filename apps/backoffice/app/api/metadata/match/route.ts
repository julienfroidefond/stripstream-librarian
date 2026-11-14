import { NextRequest, NextResponse } from "next/server";
import { apiFetch, ExternalMetadataLinkDto } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch<ExternalMetadataLinkDto>("/metadata/match", {
    method: "POST",
    body: JSON.stringify(body),
  });
  invalidateAfter({ type: "metadata.changed" });
  return NextResponse.json(data);
}, { fallback: "Failed to create metadata match" });

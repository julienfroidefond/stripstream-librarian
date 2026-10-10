import { NextRequest, NextResponse } from "next/server";
import { revalidateTag } from "next/cache";
import { apiFetch, ExternalMetadataLinkDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch<ExternalMetadataLinkDto>("/metadata/match", {
    method: "POST",
    body: JSON.stringify(body),
  });
  revalidateTag("metadata", { expire: 0 });
  return NextResponse.json(data);
}, { fallback: "Failed to create metadata match" });

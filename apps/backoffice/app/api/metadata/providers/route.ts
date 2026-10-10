import { NextResponse } from "next/server";
import { apiFetch, MetadataProviderDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch<MetadataProviderDto[]>("/metadata/providers");
  return NextResponse.json(data);
}, { fallback: "Failed to fetch metadata providers" });

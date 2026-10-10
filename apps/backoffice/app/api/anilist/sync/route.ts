import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async () => {
  const data = await apiFetch("/anilist/sync", { method: "POST", body: "{}" });
  return NextResponse.json(data);
}, { fallback: "Failed to sync to AniList" });

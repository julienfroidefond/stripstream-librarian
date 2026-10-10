import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async () => {
  const data = await apiFetch("/anilist/pull", { method: "POST", body: "{}" });
  return NextResponse.json(data);
}, { fallback: "Failed to pull from AniList" });

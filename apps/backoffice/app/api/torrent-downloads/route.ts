import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch("/torrent-downloads");
  return NextResponse.json(data);
}, { message: "Failed to fetch torrent downloads" });

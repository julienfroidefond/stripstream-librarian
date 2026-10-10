import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch("/download-detection/latest-found");
  return NextResponse.json(data);
}, { message: "Failed to fetch latest detection results" });

import { NextResponse } from "next/server";
import { getCacheStats } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await getCacheStats();
  return NextResponse.json(data);
}, { message: "Failed to fetch cache stats" });

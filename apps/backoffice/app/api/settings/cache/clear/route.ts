import { NextResponse } from "next/server";
import { clearCache } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async () => {
  const data = await clearCache();
  return NextResponse.json(data);
}, { message: "Failed to clear cache" });

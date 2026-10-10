import { NextRequest, NextResponse } from "next/server";
import { fetchStats } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const period = request.nextUrl.searchParams.get("period");
  const validPeriod = period === "day" ? "day" : period === "month" ? "month" : "week";
  const data = await fetchStats(validPeriod);
  return NextResponse.json(data);
}, { message: "Failed to fetch stats" });

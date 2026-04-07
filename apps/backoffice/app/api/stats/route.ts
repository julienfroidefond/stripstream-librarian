import { NextRequest, NextResponse } from "next/server";
import { fetchStats } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const period = request.nextUrl.searchParams.get("period");
    const validPeriod = period === "day" ? "day" : period === "month" ? "month" : "week";
    const data = await fetchStats(validPeriod);
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ error: "Failed to fetch stats" }, { status: 500 });
  }
}

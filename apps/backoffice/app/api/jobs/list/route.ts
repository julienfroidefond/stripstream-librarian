import { NextResponse } from "next/server";
import { listJobs } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await listJobs();
  return NextResponse.json(data);
}, { message: "Failed to fetch jobs" });

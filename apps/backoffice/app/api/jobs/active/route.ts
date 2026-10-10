import { NextResponse } from "next/server";
import { apiFetch, IndexJobDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch<IndexJobDto[]>("/index/jobs/active");
  return NextResponse.json(data);
}, { message: "Failed to fetch active jobs" });

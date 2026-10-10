import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import type { UserReadingOverviewDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch<UserReadingOverviewDto[]>("/admin/reading-overview");
  return NextResponse.json(data);
}, { fallback: "Failed" });

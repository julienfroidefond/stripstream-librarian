import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import type { UserReadingOverviewSeriesBookDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ userId: string }> },
) => {
  const { userId } = await params;
  const qs = request.nextUrl.search;
  const data = await apiFetch<UserReadingOverviewSeriesBookDto[]>(`/admin/reading-overview/${userId}/series-books${qs}`);
  return NextResponse.json(data);
}, { fallback: "Failed" });

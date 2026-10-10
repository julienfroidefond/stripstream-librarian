import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type SeriesPage = {
  items: unknown[];
  total: number;
  page: number;
  limit: number;
};

export const GET = withRoute(
  async (request: NextRequest) => {
    const { searchParams } = request.nextUrl;
    const data = await apiFetch<SeriesPage>(`/series?${searchParams.toString()}`);
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to search series",
    onError: (error) => console.error("[series/search] Error:", error),
  }
);

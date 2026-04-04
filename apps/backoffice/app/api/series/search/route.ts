import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

type SeriesPage = {
  items: unknown[];
  total: number;
  page: number;
  limit: number;
};

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const data = await apiFetch<SeriesPage>(`/series?${searchParams.toString()}`);
    return NextResponse.json(data);
  } catch (error) {
    console.error("[series/search] Error:", error);
    const message = error instanceof Error ? error.message : "Failed to search series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

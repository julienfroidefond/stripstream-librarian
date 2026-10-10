import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (req: NextRequest) => {
  const libraryId = req.nextUrl.searchParams.get("library_id");
  const qs = libraryId ? `?library_id=${libraryId}` : "";
  const data = await apiFetch<{ name: string; series_count: number }[]>(`/genres${qs}`);
  return NextResponse.json(data);
}, { fallback: "Failed" });

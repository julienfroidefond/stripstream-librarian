import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(req: NextRequest) {
  try {
    const libraryId = req.nextUrl.searchParams.get("library_id");
    const qs = libraryId ? `?library_id=${libraryId}` : "";
    const data = await apiFetch<{ name: string; series_count: number }[]>(`/genres${qs}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

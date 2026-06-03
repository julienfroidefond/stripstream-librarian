import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import type { ArchivedSeriesItemDto } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch<ArchivedSeriesItemDto[]>("/admin/series/archived");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

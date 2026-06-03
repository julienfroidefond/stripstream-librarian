import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import type { UserReadingOverviewDto } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch<UserReadingOverviewDto[]>("/admin/reading-overview");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

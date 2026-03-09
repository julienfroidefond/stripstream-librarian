import { NextResponse } from "next/server";
import { apiFetch, IndexJobDto } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch<IndexJobDto[]>("/index/jobs/active");
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to fetch active jobs" }, { status: 500 });
  }
}

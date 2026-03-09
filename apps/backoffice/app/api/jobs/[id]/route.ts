import { NextRequest, NextResponse } from "next/server";
import { apiFetch, IndexJobDto } from "@/lib/api";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const data = await apiFetch<IndexJobDto>(`/index/jobs/${id}`);
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to fetch job" }, { status: 500 });
  }
}

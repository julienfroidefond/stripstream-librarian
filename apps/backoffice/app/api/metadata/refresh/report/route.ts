import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const jobId = request.nextUrl.searchParams.get("job_id");
    if (!jobId) {
      return NextResponse.json({ error: "job_id required" }, { status: 400 });
    }
    const data = await apiFetch(`/metadata/refresh/${jobId}/report`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to get report";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

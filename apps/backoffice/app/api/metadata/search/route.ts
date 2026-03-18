import { NextRequest, NextResponse } from "next/server";
import { apiFetch, SeriesCandidateDto } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch<SeriesCandidateDto[]>("/metadata/search", {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to search metadata";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

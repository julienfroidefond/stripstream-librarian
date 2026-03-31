import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ seriesId: string }>;

export async function POST(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const body = await request.json();
    const data = await apiFetch(`/series/${seriesId}/rename-books`, {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to rename books";
    console.error("[rename-books] proxy error:", message);
    // Try to extract HTTP status from the error message (format: "API ... failed (STATUS): ...")
    const statusMatch = message.match(/failed \((\d+)\)/);
    const status = statusMatch ? parseInt(statusMatch[1]) : 500;
    return NextResponse.json({ error: message }, { status });
  }
}

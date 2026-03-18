import { NextRequest, NextResponse } from "next/server";
import { apiFetch, MissingBooksDto } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");
    if (!id) {
      return NextResponse.json({ error: "id is required" }, { status: 400 });
    }
    const data = await apiFetch<MissingBooksDto>(`/metadata/missing/${id}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch missing books";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

import { NextRequest, NextResponse } from "next/server";
import { apiFetch, MetadataBatchResultDto } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");
    if (!id) {
      return NextResponse.json({ error: "id is required" }, { status: 400 });
    }
    const status = searchParams.get("status") || "";
    const params = status ? `?status=${status}` : "";
    const data = await apiFetch<MetadataBatchResultDto[]>(`/metadata/batch/${id}/results${params}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch results";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

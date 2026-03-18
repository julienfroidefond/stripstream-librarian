import { NextRequest, NextResponse } from "next/server";
import { apiFetch, ExternalMetadataLinkDto } from "@/lib/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const libraryId = searchParams.get("library_id") || "";
    const seriesName = searchParams.get("series_name") || "";
    const params = new URLSearchParams();
    if (libraryId) params.set("library_id", libraryId);
    if (seriesName) params.set("series_name", seriesName);
    const data = await apiFetch<ExternalMetadataLinkDto[]>(`/metadata/links?${params.toString()}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch metadata links";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(request: NextRequest) {
  try {
    const { searchParams } = new URL(request.url);
    const id = searchParams.get("id");
    if (!id) {
      return NextResponse.json({ error: "id is required" }, { status: 400 });
    }
    const data = await apiFetch<{ deleted: boolean }>(`/metadata/links/${id}`, {
      method: "DELETE",
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to delete metadata link";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

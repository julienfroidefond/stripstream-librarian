import { NextResponse } from "next/server";
import { apiFetch, MetadataProviderDto } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch<MetadataProviderDto[]>("/metadata/providers");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch metadata providers";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

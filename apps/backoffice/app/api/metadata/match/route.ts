import { NextRequest, NextResponse } from "next/server";
import { revalidateTag } from "next/cache";
import { apiFetch, ExternalMetadataLinkDto } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch<ExternalMetadataLinkDto>("/metadata/match", {
      method: "POST",
      body: JSON.stringify(body),
    });
    revalidateTag("metadata", { expire: 0 });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to create metadata match";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

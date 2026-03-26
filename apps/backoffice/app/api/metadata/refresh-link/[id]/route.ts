import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function POST(_request: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  try {
    const { id } = await params;
    const data = await apiFetch(`/metadata/refresh-link/${id}`, { method: "POST" });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to refresh metadata";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

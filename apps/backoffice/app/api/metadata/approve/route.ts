import { NextRequest, NextResponse } from "next/server";
import { revalidateTag } from "next/cache";
import { apiFetch } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { id, ...rest } = body;
    const data = await apiFetch<{ status: string; books_synced: number }>(`/metadata/approve/${id}`, {
      method: "POST",
      body: JSON.stringify(rest),
    });
    revalidateTag("metadata", { expire: 0 });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to approve metadata";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

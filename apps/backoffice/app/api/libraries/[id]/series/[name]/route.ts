import { NextRequest, NextResponse } from "next/server";
import { updateSeries } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string; name: string }> }
) {
  const { id, name } = await params;
  try {
    const body = await request.json();
    const data = await updateSeries(id, name, body);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

import { NextRequest, NextResponse } from "next/server";
import { updateSeries, deleteSeries } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string; seriesId: string }> }
) {
  const { id, seriesId } = await params;
  try {
    const body = await request.json();
    const data = await updateSeries(id, seriesId, body);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string; seriesId: string }> }
) {
  const { id, seriesId } = await params;
  try {
    await deleteSeries(id, seriesId);
    return NextResponse.json({ deleted: true });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to delete series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

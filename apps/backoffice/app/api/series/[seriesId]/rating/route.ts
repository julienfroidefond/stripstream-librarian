import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ seriesId: string }>;

export async function PUT(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const body = await request.json();
    await apiFetch(`/series/${seriesId}/rating`, {
      method: "PUT",
      body: JSON.stringify(body),
    });
    revalidatePath(`/series/${seriesId}`);
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to save rating";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(_request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    await apiFetch(`/series/${seriesId}/rating`, { method: "DELETE" });
    revalidatePath(`/series/${seriesId}`);
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to delete rating";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

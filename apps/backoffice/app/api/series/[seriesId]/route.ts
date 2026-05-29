import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ seriesId: string }>;

export async function PATCH(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const body = await request.json();
    const data = await apiFetch(`/series/${seriesId}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(request: NextRequest, { params }: { params: Params }) {
  try {
    const { seriesId } = await params;
    const data = await apiFetch(`/series/${seriesId}`, { method: "DELETE" });
    revalidatePath("/series");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to delete series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

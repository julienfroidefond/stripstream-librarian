import { revalidatePath } from "next/cache";
import { NextRequest, NextResponse } from "next/server";
import { apiFetch, LibraryDto } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const body = await request.json();
    const data = await apiFetch<LibraryDto>(`/libraries/${id}/metadata-provider`, {
      method: "PATCH",
      body: JSON.stringify(body),
    });
    revalidatePath("/libraries");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update metadata provider";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

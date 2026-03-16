import { NextRequest, NextResponse } from "next/server";
import { updateBook } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;
  try {
    const body = await request.json();
    const data = await updateBook(bookId, body);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update book";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

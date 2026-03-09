import { NextRequest, NextResponse } from "next/server";
import { convertBook } from "@/lib/api";

export async function POST(
  _request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;
  try {
    const data = await convertBook(bookId);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to start conversion";
    const status = message.includes("409") ? 409 : 500;
    return NextResponse.json({ error: message }, { status });
  }
}

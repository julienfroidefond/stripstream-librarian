import { NextRequest, NextResponse } from "next/server";
import { revalidatePath } from "next/cache";
import { updateReadingProgress, apiFetch, BookDto } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;
  try {
    const body = await request.json();
    const [data, book] = await Promise.all([
      updateReadingProgress(bookId, body.status, body.current_page ?? undefined),
      apiFetch<BookDto>(`/books/${bookId}`).catch(() => null),
    ]);
    revalidatePath("/series");
    revalidatePath(`/books/${bookId}`);
    if (book?.series_id) {
      revalidatePath(`/series/${book.series_id}`);
    }
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update reading progress";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

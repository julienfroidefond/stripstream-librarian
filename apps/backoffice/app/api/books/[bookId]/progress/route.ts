import { NextRequest, NextResponse } from "next/server";
import { revalidatePath } from "next/cache";
import { updateReadingProgress, apiFetch, BookDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
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
}, { fallback: "Failed to update reading progress" });

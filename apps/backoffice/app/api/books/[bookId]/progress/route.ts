import { NextRequest, NextResponse } from "next/server";
import { updateReadingProgress, apiFetch, BookDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
  const body = await request.json();
  const [data, book] = await Promise.all([
    updateReadingProgress(bookId, body.status, body.current_page ?? undefined),
    apiFetch<BookDto>(`/books/${bookId}`).catch(() => null),
  ]);
  invalidateAfter({ type: "readingProgress.changed", bookId, seriesId: book?.series_id });
  return NextResponse.json(data);
}, { fallback: "Failed to update reading progress" });

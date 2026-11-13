import { NextRequest, NextResponse } from "next/server";
import { updateBook, apiFetch, BookDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
  const body = await request.json();
  const data = await updateBook(bookId, body);
  invalidateAfter({ type: "book.updated", bookId, seriesId: data?.series_id });
  return NextResponse.json(data);
}, { fallback: "Failed to update book" });

export const DELETE = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
  // Resolve the series before the book is gone so the series pages can be revalidated.
  const book = await apiFetch<BookDto>(`/books/${bookId}`).catch(() => null);
  const data = await apiFetch(`/books/${bookId}`, { method: "DELETE" });
  invalidateAfter({ type: "book.deleted", bookId, seriesId: book?.series_id });
  return NextResponse.json(data);
}, { fallback: "Failed to delete book" });

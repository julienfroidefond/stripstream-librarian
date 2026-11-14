import { NextRequest, NextResponse } from "next/server";
import { updateBook, apiFetch, type BookDetailsDto } from "@/lib/api";
import { withRoute } from "@/lib/server/api-handler";
import { invalidateAfter } from "@/lib/cache";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
  const data = await apiFetch<BookDetailsDto>(`/books/${encodeURIComponent(bookId)}`);
  return NextResponse.json(data);
}, {
  fallback: "Failed to fetch book",
  statusFromError: (_error, message) => (message.includes("(404)") ? 404 : undefined),
});

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
  const book = await apiFetch<BookDetailsDto>(`/books/${bookId}`).catch(() => null);
  const data = await apiFetch(`/books/${bookId}`, { method: "DELETE" });
  invalidateAfter({ type: "book.deleted", bookId, seriesId: book?.series_id });
  return NextResponse.json(data);
}, { fallback: "Failed to delete book" });

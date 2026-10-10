import { NextRequest, NextResponse } from "next/server";
import { updateBook, apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const PATCH = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
  const body = await request.json();
  const data = await updateBook(bookId, body);
  return NextResponse.json(data);
}, { fallback: "Failed to update book" });

export const DELETE = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }) => {
  const { bookId } = await params;
  const data = await apiFetch(`/books/${bookId}`, { method: "DELETE" });
  return NextResponse.json(data);
}, { fallback: "Failed to delete book" });

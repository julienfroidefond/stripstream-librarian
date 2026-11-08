import { NextRequest, NextResponse } from "next/server";
import { revalidatePath, revalidateTag } from "next/cache";
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
  // Le dashboard met en cache ses statistiques 30 s ; une progression de
  // lecture doit s'y refléter immédiatement (listes « en cours » / « récents »).
  revalidateTag("stats", { expire: 0 });
  revalidatePath("/");
  if (book?.series_id) {
    revalidatePath(`/series/${book.series_id}`);
  }
  return NextResponse.json(data);
}, { fallback: "Failed to update reading progress" });

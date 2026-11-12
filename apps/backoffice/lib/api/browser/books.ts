/** Browser-side book mutations and reading progress. */

import type { BookDto, ReadingProgressDto, UpdateBookRequest } from "../books";
import { browserFetch, browserFetchVoid } from "./transport";

export type { UpdateBookRequest } from "../books";

export const booksApi = {
  update(bookId: string, body: UpdateBookRequest): Promise<BookDto> {
    return browserFetch<BookDto>(`/api/books/${encodeURIComponent(bookId)}`, {
      method: "PATCH",
      body,
    });
  },

  convert(bookId: string): Promise<{ id: string }> {
    return browserFetch<{ id: string }>(`/api/books/${encodeURIComponent(bookId)}/convert`, {
      method: "POST",
    });
  },

  markProgress(
    bookId: string,
    status: "read" | "unread" | "reading",
    currentPage?: number
  ): Promise<ReadingProgressDto> {
    return browserFetch<ReadingProgressDto>(
      `/api/books/${encodeURIComponent(bookId)}/progress`,
      {
        method: "PATCH",
        body: currentPage === undefined ? { status } : { status, current_page: currentPage },
      }
    );
  },

  /** Remove a book row (the API also removes the file). */
  remove(bookId: string): Promise<void> {
    return browserFetchVoid(`/api/books/${encodeURIComponent(bookId)}`, { method: "DELETE" });
  },
};

"use client";

import { DeleteConfirmButton } from "./DeleteConfirmButton";

interface DeleteBookButtonProps {
  bookId: string;
  seriesId?: string;
  children?: (open: () => void) => React.ReactNode;
}

export function DeleteBookButton({ bookId, seriesId, children }: DeleteBookButtonProps) {
  return (
    <DeleteConfirmButton
      endpoint={`/api/books/${bookId}`}
      redirectTo={seriesId ? `/series/${seriesId}` : "/series"}
      titleKey="bookDetail.delete"
      confirmKey="bookDetail.confirmDelete"
      labelKey="bookDetail.delete"
    >
      {children}
    </DeleteConfirmButton>
  );
}

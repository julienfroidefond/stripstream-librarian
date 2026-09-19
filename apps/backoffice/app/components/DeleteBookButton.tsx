"use client";

import { DeleteConfirmButton } from "./DeleteConfirmButton";

interface DeleteBookButtonProps {
  bookId: string;
  libraryId?: string;
  children?: (open: () => void) => React.ReactNode;
}

export function DeleteBookButton({ bookId, children }: DeleteBookButtonProps) {
  return (
    <DeleteConfirmButton
      endpoint={`/api/books/${bookId}`}
      titleKey="bookDetail.delete"
      confirmKey="bookDetail.confirmDelete"
      labelKey="bookDetail.delete"
    >
      {children}
    </DeleteConfirmButton>
  );
}

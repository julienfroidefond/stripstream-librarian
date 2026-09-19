"use client";

import { DeleteConfirmButton } from "./DeleteConfirmButton";

interface DeleteSeriesButtonProps {
  seriesId: string;
  children?: (open: () => void) => React.ReactNode;
}

export function DeleteSeriesButton({ seriesId, children }: DeleteSeriesButtonProps) {
  return (
    <DeleteConfirmButton
      endpoint={`/api/series/${seriesId}`}
      titleKey="seriesDetail.delete"
      confirmKey="seriesDetail.confirmDelete"
      labelKey="seriesDetail.delete"
    >
      {children}
    </DeleteConfirmButton>
  );
}

"use client";

import { useState } from "react";
import {
  BooksGridWithMissing,
  BooksListWithMissing,
  type MissingBook,
} from "./BookCard";
import { BookDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import type { ViewMode } from "@/lib/viewMode";
import { ViewModeToggle } from "./ViewModeToggle";
import { Button, Icon } from "./ui";

/**
 * Books of a series, with the dashed missing-volume entries mixed in and a
 * toggle to hide them. Honours the grid/list preference of the surrounding
 * page so the series detail follows the same view mode as the book lists.
 */
export function BooksGridWithMissingToggle({
  books,
  missingBooks,
  compact,
  hasActiveUser = true,
  viewMode = "grid",
}: {
  books: (BookDto & { coverUrl?: string })[];
  missingBooks: MissingBook[];
  compact?: boolean;
  hasActiveUser?: boolean;
  viewMode?: ViewMode;
}) {
  const { t } = useTranslation();
  const [showMissing, setShowMissing] = useState(true);

  return (
    <div>
      <div className="mb-3 flex items-center justify-end gap-2">
        {missingBooks.length > 0 && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowMissing(!showMissing)}
            className="text-xs gap-1.5"
          >
            <Icon name="eye" size="sm" />
            {showMissing
              ? t("series.hideMissing")
              : t("series.showMissing", { count: missingBooks.length })}
          </Button>
        )}
        <ViewModeToggle viewMode={viewMode} />
      </div>
      {viewMode === "list" ? (
        <BooksListWithMissing
          books={books}
          missingBooks={missingBooks}
          showMissing={showMissing}
          hasActiveUser={hasActiveUser}
        />
      ) : (
        <BooksGridWithMissing
          books={books}
          missingBooks={missingBooks}
          showMissing={showMissing}
          compact={compact}
          hasActiveUser={hasActiveUser}
        />
      )}
    </div>
  );
}

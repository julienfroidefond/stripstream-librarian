"use client";

import { useState } from "react";
import { BooksGridWithMissing, type MissingBook } from "./BookCard";
import { BookDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import { Button, Icon } from "./ui";

export function BooksGridWithMissingToggle({
  books,
  missingBooks,
  compact,
  hasActiveUser = true,
}: {
  books: (BookDto & { coverUrl?: string })[];
  missingBooks: MissingBook[];
  compact?: boolean;
  hasActiveUser?: boolean;
}) {
  const { t } = useTranslation();
  const [showMissing, setShowMissing] = useState(true);

  return (
    <div>
      {missingBooks.length > 0 && (
        <div className="flex justify-end mb-3">
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
        </div>
      )}
      <BooksGridWithMissing
        books={books}
        missingBooks={missingBooks}
        showMissing={showMissing}
        compact={compact}
        hasActiveUser={hasActiveUser}
      />
    </div>
  );
}

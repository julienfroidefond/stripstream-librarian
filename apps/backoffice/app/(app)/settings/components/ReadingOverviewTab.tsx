"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import Image from "next/image";
import { Card, CardHeader, CardTitle, CardContent, Icon } from "@/app/components/ui";
import { getBookCoverUrl } from "@/lib/api";
import type { UserReadingOverviewDto, UserReadingOverviewItemDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

function StatPill({ value, label, color }: { value: number | string; label: string; color: string }) {
  return (
    <div className="flex flex-col items-center px-3 py-2 rounded-lg bg-muted/40">
      <span className={`text-xl font-bold ${color}`}>{value}</span>
      <span className="text-[10px] text-muted-foreground mt-0.5">{label}</span>
    </div>
  );
}

function BookRow({ book, showProgress }: { book: UserReadingOverviewItemDto; showProgress: boolean }) {
  const { t } = useTranslation();
  const pct = book.page_count > 0 ? Math.round((book.current_page / book.page_count) * 100) : 0;

  return (
    <Link
      href={`/books/${book.book_id}` as any}
      className="flex items-center gap-3 group rounded-md hover:bg-muted/40 p-1 -mx-1 transition-colors"
    >
      <Image
        src={getBookCoverUrl(book.book_id)}
        alt={book.title}
        width={32}
        height={44}
        className="w-8 h-11 object-cover rounded shadow-sm shrink-0 bg-muted"
      />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-foreground truncate group-hover:text-primary transition-colors">
          {book.title}
        </p>
        {book.series && (
          <p className="text-xs text-muted-foreground truncate">{book.series}</p>
        )}
        {showProgress ? (
          <div className="flex items-center gap-2 mt-1">
            <div className="h-1 flex-1 bg-muted rounded-full overflow-hidden">
              <div className="h-full bg-warning rounded-full" style={{ width: `${pct}%` }} />
            </div>
            <span className="text-[10px] text-muted-foreground shrink-0">
              {t("settings.readingOverview.page")}{book.current_page}/{book.page_count}
            </span>
          </div>
        ) : (
          book.last_read_at && (
            <p className="text-[10px] text-muted-foreground mt-0.5">{book.last_read_at}</p>
          )
        )}
      </div>
      {!showProgress && (
        <Icon name="check" size="sm" className="text-success shrink-0" />
      )}
    </Link>
  );
}

function UserCard({ user }: { user: UserReadingOverviewDto }) {
  const { t } = useTranslation();
  const total = user.books_read + user.books_reading;
  const pct = total > 0 ? Math.round((user.books_read / total) * 100) : 0;

  return (
    <Card className="mb-4">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <span className="w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-xs shrink-0">
            {user.username.charAt(0).toUpperCase()}
          </span>
          {user.username}
          {user.last_read_at && (
            <span className="ml-auto text-xs text-muted-foreground font-normal">
              {t("settings.readingOverview.lastReadAt")} : {user.last_read_at}
            </span>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {/* Stats pills */}
        <div className="flex gap-3 mb-4">
          <StatPill value={user.books_read} label={t("settings.readingOverview.booksRead")} color="text-success" />
          <StatPill value={user.books_reading} label={t("settings.readingOverview.booksReading")} color="text-warning" />
          <StatPill value={user.series_in_progress} label={t("settings.readingOverview.seriesInProgress")} color="text-primary" />
        </div>

        {/* Global progress bar */}
        {total > 0 && (
          <div className="mb-4">
            <div className="flex justify-between text-xs text-muted-foreground mb-1">
              <span>{user.books_read}/{total}</span>
              <span>{pct}%</span>
            </div>
            <div className="h-2 bg-muted rounded-full overflow-hidden">
              <div className="h-full bg-success rounded-full transition-all" style={{ width: `${pct}%` }} />
            </div>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Currently reading */}
          {user.currently_reading.length > 0 && (
            <div>
              <p className="text-xs font-medium text-muted-foreground mb-2 uppercase tracking-wide">
                {t("settings.readingOverview.currentlyReading")}
              </p>
              <div className="space-y-1">
                {user.currently_reading.map((book) => (
                  <BookRow key={book.book_id} book={book} showProgress={true} />
                ))}
              </div>
            </div>
          )}

          {/* Recently read */}
          {user.recently_read.length > 0 && (
            <div>
              <p className="text-xs font-medium text-muted-foreground mb-2 uppercase tracking-wide">
                {t("settings.readingOverview.recentlyRead")}
              </p>
              <div className="space-y-1 max-h-80 overflow-y-auto pr-1">
                {user.recently_read.map((book) => (
                  <BookRow key={book.book_id} book={book} showProgress={false} />
                ))}
              </div>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export function ReadingOverviewTab() {
  const { t } = useTranslation();
  const [overview, setOverview] = useState<UserReadingOverviewDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/reading-overview")
      .then((r) => r.json())
      .then((data) => setOverview(data))
      .catch(() => setError("Erreur de chargement"));
  }, []);

  if (error) {
    return <p className="text-destructive text-sm">{error}</p>;
  }

  if (!overview) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground text-sm">
        <Icon name="spinner" size="sm" className="animate-spin" />
        Chargement…
      </div>
    );
  }

  if (overview.length === 0) {
    return (
      <p className="text-muted-foreground text-sm text-center py-8">
        {t("settings.readingOverview.noActivity")}
      </p>
    );
  }

  return (
    <div>
      {overview.map((user) => (
        <UserCard key={user.user_id} user={user} />
      ))}
    </div>
  );
}

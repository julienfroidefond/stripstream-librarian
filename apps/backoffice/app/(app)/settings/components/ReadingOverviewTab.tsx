"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import Image from "next/image";
import { Icon } from "@/app/components/ui";
import { getBookCoverUrl } from "@/lib/api";
import type { UserReadingOverviewDto, UserReadingOverviewItemDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

function CurrentlyReadingCard({ book }: { book: UserReadingOverviewItemDto }) {
  const { t } = useTranslation();
  const pct = book.page_count > 0 ? Math.round((book.current_page / book.page_count) * 100) : 0;

  return (
    <Link
      href={`/books/${book.book_id}` as any}
      className="group flex flex-col w-28 shrink-0"
    >
      <div className="relative">
        <Image
          src={getBookCoverUrl(book.book_id)}
          alt={book.title}
          width={112}
          height={160}
          className="w-28 h-40 object-cover rounded-md shadow-md bg-muted group-hover:shadow-lg transition-shadow"
        />
        <div className="absolute bottom-0 left-0 right-0 h-1.5 bg-black/30 rounded-b-md overflow-hidden">
          <div className="h-full bg-warning transition-all" style={{ width: `${pct}%` }} />
        </div>
        <span className="absolute bottom-2 right-1.5 text-[10px] font-bold text-white drop-shadow">{pct}%</span>
      </div>
      <p className="text-xs font-medium text-foreground truncate mt-1.5 group-hover:text-primary transition-colors">
        {book.title}
      </p>
      {book.series && (
        <p className="text-[10px] text-muted-foreground truncate">{book.series}</p>
      )}
      <p className="text-[10px] text-muted-foreground/70 mt-0.5">
        {t("settings.readingOverview.page")}{book.current_page}/{book.page_count}
      </p>
    </Link>
  );
}

function RecentlyReadRow({ book }: { book: UserReadingOverviewItemDto }) {
  return (
    <Link
      href={`/books/${book.book_id}` as any}
      className="flex items-center gap-2.5 group rounded-md hover:bg-muted/50 px-2 py-1.5 -mx-2 transition-colors"
    >
      <Image
        src={getBookCoverUrl(book.book_id)}
        alt={book.title}
        width={28}
        height={40}
        className="w-7 h-10 object-cover rounded shadow-sm shrink-0 bg-muted"
      />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-foreground truncate group-hover:text-primary transition-colors leading-tight">
          {book.title}
        </p>
        {book.series && (
          <p className="text-[10px] text-muted-foreground truncate">{book.series}</p>
        )}
      </div>
      <div className="flex items-center gap-1.5 shrink-0">
        {book.last_read_at && (
          <span className="text-[10px] text-muted-foreground">{book.last_read_at}</span>
        )}
        <Icon name="check" size="sm" className="text-success" />
      </div>
    </Link>
  );
}

function UserCard({ user }: { user: UserReadingOverviewDto }) {
  const { t } = useTranslation();
  const total = user.books_read + user.books_reading;
  const pct = total > 0 ? Math.round((user.books_read / total) * 100) : 0;
  const recentlyRead = user.recently_read ?? [];

  return (
    <div className="mb-6 rounded-xl border border-border bg-card overflow-hidden shadow-sm">
      {/* Header */}
      <div className="px-5 pt-4 pb-4 border-b border-border/60 bg-muted/20">
        <div className="flex items-center gap-3 mb-3">
          <span className="w-9 h-9 rounded-full bg-primary/15 flex items-center justify-center text-primary font-bold text-sm shrink-0">
            {user.username.charAt(0).toUpperCase()}
          </span>
          <div className="flex-1 min-w-0">
            <p className="font-semibold text-foreground leading-tight">{user.username}</p>
            <p className="text-xs text-muted-foreground mt-0.5">
              <span className="text-success font-medium">{user.books_read}</span>
              {" "}{t("settings.readingOverview.booksRead")}
              {user.books_reading > 0 && (
                <>
                  {" · "}
                  <span className="text-warning font-medium">{user.books_reading}</span>
                  {" "}{t("settings.readingOverview.booksReading")}
                </>
              )}
              {user.series_in_progress > 0 && (
                <>
                  {" · "}
                  <span className="text-primary font-medium">{user.series_in_progress}</span>
                  {" "}{t("settings.readingOverview.seriesInProgress")}
                </>
              )}
              {user.last_read_at && (
                <span className="text-muted-foreground/60">
                  {" · "}{t("settings.readingOverview.lastReadAt")} {user.last_read_at}
                </span>
              )}
            </p>
          </div>
        </div>

        {/* Progress bar */}
        {total > 0 && (
          <div className="flex items-center gap-2">
            <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
              <div className="h-full bg-success rounded-full transition-all" style={{ width: `${pct}%` }} />
            </div>
            <span className="text-xs text-muted-foreground tabular-nums w-8 text-right">{pct}%</span>
          </div>
        )}
      </div>

      <div className="px-5 py-4 space-y-5">
        {/* Currently reading — horizontal shelf */}
        {user.currently_reading.length > 0 && (
          <div>
            <p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider mb-3">
              {t("settings.readingOverview.currentlyReading")}
            </p>
            <div className="flex gap-3 overflow-x-auto pb-1 scrollbar-thin">
              {user.currently_reading.map((book) => (
                <CurrentlyReadingCard key={book.book_id} book={book} />
              ))}
            </div>
          </div>
        )}

        {/* Recently read — compact grid */}
        {recentlyRead.length > 0 && (
          <div>
            <p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">
              {t("settings.readingOverview.recentlyRead")}
            </p>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-6 max-h-72 overflow-y-auto">
              {recentlyRead.map((book) => (
                <RecentlyReadRow key={book.book_id} book={book} />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
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

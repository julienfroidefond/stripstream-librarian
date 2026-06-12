"use client";

import React from "react";
import { useEffect, useState } from "react";
import Link from "next/link";
import Image from "next/image";
import { Badge, Icon } from "@/app/components/ui";
import { getBookCoverUrl } from "@/lib/api";
import type {
  ReadingStatus,
  UserReadingOverviewDto,
  UserReadingOverviewItemDto,
  UserReadingOverviewSeriesBookDto,
  UserReadingOverviewSeriesDto,
} from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

function volumeLabel(volume: number | null, volumeType: string): string {
  if (volumeType === "hs") return volume != null ? `HS ${volume}` : "HS";
  if (volumeType === "oneshot") return "OS";
  if (volumeType === "integral") return volume != null ? `Int ${volume}` : "Int";
  return volume != null ? `T${volume}` : "—";
}

function readingStatusBadgeVariant(status: ReadingStatus): "success" | "warning" | "secondary" {
  if (status === "read") return "success";
  if (status === "reading") return "warning";
  return "secondary";
}

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

function ReadingStatusBadge({ status, currentPage }: { status: ReadingStatus; currentPage: number }) {
  const { t } = useTranslation();

  return (
    <Badge variant={readingStatusBadgeVariant(status)}>
      {status === "reading" && currentPage > 0
        ? `${t("status.reading")} · p.${currentPage}`
        : t(`status.${status}` as const)}
    </Badge>
  );
}

function SeriesBooksTable({ books }: { books: UserReadingOverviewSeriesBookDto[] }) {
  const { t } = useTranslation();

  return (
    <div className="overflow-x-auto rounded-lg border border-border/50 bg-background">
      <table className="w-full min-w-[560px] text-sm">
        <thead>
          <tr className="border-b border-border/50 text-muted-foreground">
            <th className="text-left px-4 py-2 font-medium">{t("settings.readingOverview.bookCover")}</th>
            <th className="text-left px-4 py-2 font-medium">{t("settings.readingOverview.bookVolume")}</th>
            <th className="text-left px-4 py-2 font-medium">{t("settings.readingOverview.bookTitle")}</th>
            <th className="text-left px-4 py-2 font-medium">{t("settings.readingOverview.bookStatus")}</th>
            <th className="text-right px-4 py-2 font-medium">{t("settings.readingOverview.bookProgress")}</th>
            <th className="text-right px-4 py-2 font-medium">{t("settings.readingOverview.updatedAt")}</th>
          </tr>
        </thead>
        <tbody>
          {books.map((book) => (
            <tr key={book.book_id} className="border-b border-border/30 last:border-0">
              <td className="px-4 py-2">
                <Link href={`/books/${book.book_id}` as any} className="block w-10">
                  <Image
                    src={getBookCoverUrl(book.book_id)}
                    alt={book.title}
                    width={40}
                    height={56}
                    className="w-10 h-14 object-cover rounded-md bg-muted shadow-sm"
                  />
                </Link>
              </td>
              <td className="px-4 py-2 whitespace-nowrap text-muted-foreground">
                {volumeLabel(book.volume, book.volume_type)}
              </td>
              <td className="px-4 py-2">
                <Link href={`/books/${book.book_id}` as any} className="font-medium text-foreground hover:text-primary transition-colors">
                  {book.title}
                </Link>
              </td>
              <td className="px-4 py-2">
                <ReadingStatusBadge status={book.status} currentPage={book.current_page} />
              </td>
              <td className="px-4 py-2 text-right text-muted-foreground tabular-nums whitespace-nowrap">
                {book.status === "reading" && book.page_count > 0
                  ? `${book.current_page}/${book.page_count}`
                  : book.page_count > 0
                    ? book.page_count
                    : "—"}
              </td>
              <td className="px-4 py-2 text-right text-muted-foreground whitespace-nowrap">
                {book.last_read_at ?? "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function SeriesStatusTable({ series }: { series: UserReadingOverviewSeriesDto[] }) {
  const { t } = useTranslation();
  const [expandedSeries, setExpandedSeries] = useState<string | null>(null);

  if (series.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">{t("settings.readingOverview.noSeries")}</p>
    );
  }

  return (
    <div className="overflow-x-auto rounded-xl border border-border/60">
      <table className="w-full min-w-[760px] text-sm">
        <thead>
          <tr className="border-b border-border text-muted-foreground">
            <th className="w-8 px-4 py-3" />
            <th className="text-left px-4 py-3 font-medium">{t("settings.readingOverview.series")}</th>
            <th className="text-left px-4 py-3 font-medium">{t("settings.readingOverview.statuses")}</th>
            <th className="text-right px-4 py-3 font-medium">{t("settings.readingOverview.progressColumn")}</th>
            <th className="text-right px-4 py-3 font-medium">{t("settings.readingOverview.updatedAt")}</th>
          </tr>
        </thead>
        <tbody>
          {series.map((item) => {
            const key = item.series_id ?? `unclassified-${item.series_name}`;
            const isExpanded = expandedSeries === key;
            const readPct = item.books_total > 0 ? Math.round((item.books_read / item.books_total) * 100) : 0;
            const coverBook = item.books[0] ?? null;

            return (
              <React.Fragment key={key}>
                <tr
                  className="border-b border-border/50 hover:bg-muted/40 transition-colors cursor-pointer"
                  onClick={() => setExpandedSeries(isExpanded ? null : key)}
                  >
                  <td className="px-4 py-3 text-muted-foreground">
                    <Icon
                      name="chevronRight"
                      size="sm"
                      className={`transition-transform ${isExpanded ? "rotate-90" : ""}`}
                    />
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-3 min-w-0">
                      {coverBook && (
                        <Image
                          src={getBookCoverUrl(coverBook.book_id)}
                          alt={item.series_name}
                          width={36}
                          height={52}
                          className="w-9 h-13 object-cover rounded-md bg-muted shadow-sm shrink-0"
                        />
                      )}
                      <div className="min-w-0">
                        <div className="font-medium text-foreground truncate">{item.series_name}</div>
                        <div className="text-xs text-muted-foreground mt-0.5">
                          {item.books_total} {t("settings.readingOverview.booksLabel")}
                        </div>
                      </div>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap gap-1.5">
                      <Badge variant="success">{item.books_read} {t("status.read")}</Badge>
                      <Badge variant="warning">{item.books_reading} {t("status.reading")}</Badge>
                      <Badge variant="secondary">{item.books_unread} {t("status.unread")}</Badge>
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-3">
                      <div className="w-24 h-1.5 bg-muted rounded-full overflow-hidden">
                        <div className="h-full bg-success rounded-full" style={{ width: `${readPct}%` }} />
                      </div>
                      <span className="text-muted-foreground tabular-nums whitespace-nowrap">
                        {item.books_read}/{item.books_total}
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right text-muted-foreground whitespace-nowrap">
                    {item.last_read_at ?? "—"}
                  </td>
                </tr>
                {isExpanded && (
                  <tr className="border-b border-border/50 bg-muted/20">
                    <td />
                    <td colSpan={4} className="px-4 py-4">
                      <SeriesBooksTable books={item.books} />
                    </td>
                  </tr>
                )}
              </React.Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function UserCard({ user }: { user: UserReadingOverviewDto }) {
  const { t } = useTranslation();
  const total = user.books_read + user.books_reading;
  const pct = total > 0 ? Math.round((user.books_read / total) * 100) : 0;
  const recentlyRead = user.recently_read ?? [];
  const [showSeriesTable, setShowSeriesTable] = useState(false);

  return (
    <div className="mb-6 rounded-xl border border-border bg-card overflow-hidden shadow-sm">
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

        <div className="rounded-xl border border-border/60 bg-muted/10">
          <button
            type="button"
            onClick={() => setShowSeriesTable((open) => !open)}
            className="w-full flex items-center justify-between gap-3 px-4 py-3 text-left hover:bg-muted/30 transition-colors"
          >
            <div>
              <p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">
                {t("settings.readingOverview.seriesStatuses")}
              </p>
              <p className="text-xs text-muted-foreground mt-1">
                {(user.series_progress ?? []).length} {t("settings.readingOverview.series").toLowerCase()}
              </p>
            </div>
            <Icon
              name="chevronRight"
              size="sm"
              className={`text-muted-foreground transition-transform ${showSeriesTable ? "rotate-90" : ""}`}
            />
          </button>
          {showSeriesTable && (
            <div className="px-4 pb-4">
              <SeriesStatusTable series={user.series_progress ?? []} />
            </div>
          )}
        </div>

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

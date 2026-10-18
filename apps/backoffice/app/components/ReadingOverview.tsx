"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import Image from "next/image";
import { Card, Icon, type IconName } from "@/app/components/ui";
import { JacketGrid } from "@/app/components/motion/JacketGrid";
import { getBookCoverUrl } from "@/lib/api";
import type {
  UserReadingOverviewDto,
  UserReadingOverviewItemDto,
  UserReadingOverviewSeriesBookDto,
  UserReadingOverviewSeriesDto,
} from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

const SERIES_PREVIEW_COUNT = 8;

type SeriesView = "list" | "grid";

const SERIES_VIEW_KEY = "stripstream:reading-overview:series-view";

function volumeLabel(volume: number | null, volumeType: string): string {
  if (volumeType === "hs") return volume != null ? `HS ${volume}` : "HS";
  if (volumeType === "oneshot") return "OS";
  if (volumeType === "integral") return volume != null ? `Int ${volume}` : "Int";
  return volume != null ? `T${volume}` : "—";
}

function pagePercent(currentPage: number, pageCount: number): number {
  return pageCount > 0 ? Math.round((currentPage / pageCount) * 100) : 0;
}

function SectionTitle({
  children,
  count,
  action,
}: {
  children: React.ReactNode;
  count?: number;
  action?: React.ReactNode;
}) {
  return (
    <h3 className="flex items-center gap-2 text-sm font-semibold text-foreground mb-3">
      {children}
      {count != null && <span className="text-xs font-normal text-muted-foreground tabular-nums">{count}</span>}
      {action && <span className="ml-auto shrink-0">{action}</span>}
    </h3>
  );
}

/** Series statuses view mode, persisted in localStorage across sessions. */
function readStoredSeriesView(): SeriesView {
  if (typeof window === "undefined") return "list";
  try {
    const stored = window.localStorage.getItem(SERIES_VIEW_KEY);
    if (stored === "list" || stored === "grid") return stored;
  } catch {
    // localStorage unavailable (private mode, blocked storage) — keep default
  }
  return "list";
}

function useSeriesView(): [SeriesView, (view: SeriesView) => void] {
  const [view, setView] = useState<SeriesView>(readStoredSeriesView);

  const updateView = (next: SeriesView) => {
    setView(next);
    try {
      window.localStorage.setItem(SERIES_VIEW_KEY, next);
    } catch {
      // ignore write failures
    }
  };

  return [view, updateView];
}

/** Segmented control to switch the series statuses section between list and grid. */
function SeriesViewToggle({
  view,
  onChange,
}: {
  view: SeriesView;
  onChange: (view: SeriesView) => void;
}) {
  const { t } = useTranslation();
  const options: { value: SeriesView; icon: IconName; label: string }[] = [
    { value: "list", icon: "list", label: t("settings.readingOverview.viewList") },
    { value: "grid", icon: "grid", label: t("settings.readingOverview.viewGrid") },
  ];

  return (
    <span role="group" className="inline-flex rounded-md border border-border bg-muted/40 p-0.5">
      {options.map((option) => {
        const active = view === option.value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={active}
            aria-label={option.label}
            title={option.label}
            onClick={() => onChange(option.value)}
            className={`rounded p-1.5 transition-colors ${
              active ? "bg-card text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
            }`}
          >
            <Icon name={option.icon} size="sm" />
          </button>
        );
      })}
    </span>
  );
}

function StatTile({ value, label, accent }: { value: number; label: string; accent: string }) {
  return (
    <div className="rounded-lg bg-muted/40 px-3 py-2.5 sm:px-4 sm:py-3">
      <div className={`text-xl sm:text-2xl font-bold tabular-nums leading-none ${accent}`}>{value}</div>
      <div className="text-2xs sm:text-xs text-muted-foreground mt-1.5 leading-tight">{label}</div>
    </div>
  );
}

/** Stacked bar: read (success) · reading (warning) · unread (muted track). */
function SeriesProgressBar({ read, reading, total }: { read: number; reading: number; total: number }) {
  const readPct = total > 0 ? (read / total) * 100 : 0;
  const readingPct = total > 0 ? (reading / total) * 100 : 0;

  return (
    <div className="flex h-1.5 w-full bg-muted rounded-full overflow-hidden">
      <div className="h-full bg-success" style={{ width: `${readPct}%` }} />
      <div className="h-full bg-warning" style={{ width: `${readingPct}%` }} />
    </div>
  );
}

function CurrentlyReadingCard({ book }: { book: UserReadingOverviewItemDto }) {
  const { t } = useTranslation();
  const pct = pagePercent(book.current_page, book.page_count);

  return (
    <Link href={`/books/${book.book_id}` as any} className="group block w-28 sm:w-32 shrink-0 snap-start">
      <div className="relative aspect-[2/3] rounded-lg overflow-hidden bg-muted shadow-sm ring-1 ring-border/60 group-hover:shadow-md transition-shadow">
        <Image
          src={getBookCoverUrl(book.book_id)}
          alt={book.title}
          fill
          sizes="128px"
          className="object-cover"
        />
      </div>
      <div className="mt-2 flex items-center gap-2">
        <div className="flex-1 h-1 bg-muted rounded-full overflow-hidden">
          <div className="h-full bg-warning rounded-full" style={{ width: `${pct}%` }} />
        </div>
        <span className="text-3xs text-muted-foreground tabular-nums">{pct}%</span>
      </div>
      <p className="text-xs font-medium text-foreground truncate mt-1 group-hover:text-primary transition-colors">
        {book.title}
      </p>
      <p className="text-2xs text-muted-foreground truncate">
        {book.series ?? `${t("settings.readingOverview.page")}${book.current_page}/${book.page_count}`}
      </p>
    </Link>
  );
}

function RecentlyReadRow({ book }: { book: UserReadingOverviewItemDto }) {
  return (
    <Link
      href={`/books/${book.book_id}` as any}
      className="flex min-w-0 items-center gap-3 group rounded-md hover:bg-muted/50 py-1.5 transition-colors"
    >
      <Image
        src={getBookCoverUrl(book.book_id)}
        alt={book.title}
        width={28}
        height={42}
        className="w-7 h-[42px] object-cover rounded shrink-0 bg-muted"
      />
      <div className="min-w-0 flex-1">
        <p className="text-sm text-foreground truncate group-hover:text-primary transition-colors leading-tight">
          {book.title}
        </p>
        <p className="text-2xs text-muted-foreground truncate mt-0.5">
          {[book.series, book.last_read_at].filter(Boolean).join(" · ")}
        </p>
      </div>
    </Link>
  );
}

function VolumeTile({ book }: { book: UserReadingOverviewSeriesBookDto }) {
  const pct = pagePercent(book.current_page, book.page_count);
  const isUnread = book.status === "unread";

  return (
    <Link href={`/books/${book.book_id}` as any} className="group block" title={book.title}>
      <div
        className={`relative aspect-[2/3] rounded-md overflow-hidden bg-muted ring-1 ring-border/60 ${
          isUnread ? "opacity-45 grayscale group-hover:opacity-80" : ""
        } transition-opacity`}
      >
        <Image src={getBookCoverUrl(book.book_id)} alt={book.title} fill sizes="96px" className="object-cover" />
        {book.status === "read" && (
          <span className="absolute top-1 right-1 w-4 h-4 rounded-full bg-success text-white flex items-center justify-center shadow">
            <Icon name="check" size="sm" className="w-3 h-3" />
          </span>
        )}
        {book.status === "reading" && (
          <div className="absolute bottom-0 inset-x-0 h-1 bg-black/40">
            <div className="h-full bg-warning" style={{ width: `${pct}%` }} />
          </div>
        )}
      </div>
      <p className="text-2xs text-muted-foreground mt-1 truncate tabular-nums">
        {volumeLabel(book.volume, book.volume_type)}
        {book.status === "reading" && book.page_count > 0 && (
          <span className="text-warning"> · {book.current_page}/{book.page_count}</span>
        )}
      </p>
      <p className="text-xs text-foreground truncate leading-tight group-hover:text-primary transition-colors">
        {book.title}
      </p>
    </Link>
  );
}

function useSeriesBooks(userId: string, seriesId: string | null, enabled: boolean) {
  const [books, setBooks] = useState<UserReadingOverviewSeriesBookDto[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!enabled || books) return;
    const qs = seriesId ? `?series_id=${encodeURIComponent(seriesId)}` : "";
    let cancelled = false;
    fetch(`/api/reading-overview/${userId}/series-books${qs}`)
      .then((r) => {
        if (!r.ok) throw new Error(String(r.status));
        return r.json();
      })
      .then((data: UserReadingOverviewSeriesBookDto[]) => {
        if (cancelled) return;
        setFailed(false);
        setBooks(data);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [userId, seriesId, enabled, books]);

  return { books, failed };
}

function SeriesBooksGrid({
  books,
  failed,
  booksTotal,
}: {
  books: UserReadingOverviewSeriesBookDto[] | null;
  failed: boolean;
  booksTotal: number;
}) {
  if (failed) {
    return <p className="text-xs text-destructive pt-1 pb-4">Erreur de chargement</p>;
  }

  return (
    <JacketGrid className="grid grid-cols-4 sm:grid-cols-6 xl:grid-cols-8 gap-3 pt-1 pb-4">
      {books
        ? books.map((book) => <VolumeTile key={book.book_id} book={book} />)
        : Array.from({ length: Math.max(1, Math.min(booksTotal, 8)) }, (_, i) => (
            <div key={i} data-testid="volume-skeleton">
              <div className="aspect-[2/3] rounded-md bg-muted animate-pulse" />
              <div className="h-2.5 w-8 mt-1.5 rounded bg-muted animate-pulse" />
            </div>
          ))}
    </JacketGrid>
  );
}

function SeriesRow({ item, userId }: { item: UserReadingOverviewSeriesDto; userId: string }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const { books, failed } = useSeriesBooks(userId, item.series_id, expanded);

  return (
    <li className="min-w-0 border-b border-border/50 last:border-0">
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setExpanded((open) => !open)}
        className="w-full min-w-0 flex items-center gap-3 py-2.5 text-left hover:bg-muted/40 rounded-md transition-colors"
      >
        <div className="relative w-8 h-12 shrink-0 rounded overflow-hidden bg-muted">
          {item.first_book_id && (
            <Image src={getBookCoverUrl(item.first_book_id)} alt={item.series_name} fill sizes="32px" className="object-cover" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-baseline justify-between gap-3">
            <span className="text-sm font-medium text-foreground truncate">{item.series_name}</span>
            <span className="text-xs text-muted-foreground tabular-nums shrink-0">
              {item.books_read}/{item.books_total}
            </span>
          </div>
          <div className="mt-1.5">
            <SeriesProgressBar read={item.books_read} reading={item.books_reading} total={item.books_total} />
          </div>
          <div className="mt-1 text-2xs text-muted-foreground truncate">
            {item.books_reading > 0 && (
              <span className="text-warning">{item.books_reading} {t("status.reading").toLowerCase()} · </span>
            )}
            {item.last_read_at ?? "—"}
          </div>
        </div>
        <Icon
          name="chevronRight"
          size="sm"
          className={`text-muted-foreground shrink-0 transition-transform ${expanded ? "rotate-90" : ""}`}
        />
      </button>
      {expanded && <SeriesBooksGrid books={books} failed={failed} booksTotal={item.books_total} />}
    </li>
  );
}

function SeriesCard({ item, userId }: { item: UserReadingOverviewSeriesDto; userId: string }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const { books, failed } = useSeriesBooks(userId, item.series_id, expanded);

  return (
    <>
      <li className="min-w-0">
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded((open) => !open)}
          title={item.series_name}
          className="group block w-full text-left"
        >
          <div className="relative aspect-[2/3] rounded-lg overflow-hidden bg-muted ring-1 ring-border/60 group-hover:shadow-md group-hover:-translate-y-0.5 transition-all">
            {item.first_book_id ? (
              <Image
                src={getBookCoverUrl(item.first_book_id)}
                alt={item.series_name}
                fill
                sizes="(max-width: 640px) 33vw, 160px"
                className="object-cover"
              />
            ) : (
              <div className="w-full h-full flex items-center justify-center text-muted-foreground/30">
                <Icon name="series" size="lg" />
              </div>
            )}
            <span className="absolute top-1 right-1 rounded-full bg-black/60 text-white text-3xs px-1.5 py-0.5 tabular-nums backdrop-blur-sm">
              {item.books_read}/{item.books_total}
            </span>
            <div className="absolute bottom-0 inset-x-0 bg-black/30">
              <SeriesProgressBar read={item.books_read} reading={item.books_reading} total={item.books_total} />
            </div>
          </div>
          <p className="mt-1.5 text-xs font-medium text-foreground truncate group-hover:text-primary transition-colors">
            {item.series_name}
          </p>
          {item.books_reading > 0 && (
            <p className="text-2xs text-warning truncate">
              {item.books_reading} {t("status.reading").toLowerCase()}
            </p>
          )}
        </button>
      </li>
      {expanded && (
        <li className="col-span-full min-w-0">
          <div className="rounded-lg border border-border/60 bg-muted/20 px-3">
            <SeriesBooksGrid books={books} failed={failed} booksTotal={item.books_total} />
          </div>
        </li>
      )}
    </>
  );
}

function SeriesList({
  series,
  userId,
  view,
}: {
  series: UserReadingOverviewSeriesDto[];
  userId: string;
  view: SeriesView;
}) {
  const { t } = useTranslation();
  const [showAll, setShowAll] = useState(false);

  if (series.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("settings.readingOverview.noSeries")}</p>;
  }

  const visible = showAll ? series : series.slice(0, SERIES_PREVIEW_COUNT);
  const keyFor = (item: UserReadingOverviewSeriesDto) => item.series_id ?? `unclassified-${item.series_name}`;

  return (
    <>
      {view === "grid" ? (
        <ul data-testid="series-grid" className="grid grid-cols-3 sm:grid-cols-4 gap-3 sm:gap-4">
          {visible.map((item) => (
            <SeriesCard key={keyFor(item)} item={item} userId={userId} />
          ))}
        </ul>
      ) : (
        <ul data-testid="series-list">
          {visible.map((item) => (
            <SeriesRow key={keyFor(item)} item={item} userId={userId} />
          ))}
        </ul>
      )}
      {series.length > SERIES_PREVIEW_COUNT && (
        <button
          type="button"
          onClick={() => setShowAll((all) => !all)}
          className="mt-2 text-xs font-medium text-primary hover:underline"
        >
          {showAll
            ? t("settings.readingOverview.showLess")
            : t("settings.readingOverview.showAll", { count: series.length })}
        </button>
      )}
    </>
  );
}

function UserOverview({ user }: { user: UserReadingOverviewDto }) {
  const { t } = useTranslation();
  const [seriesView, setSeriesView] = useSeriesView();
  const recentlyRead = user.recently_read ?? [];
  const seriesProgress = user.series_progress ?? [];

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-3 gap-2 sm:gap-3">
        <StatTile value={user.books_read} label={t("settings.readingOverview.booksRead")} accent="text-success" />
        <StatTile value={user.books_reading} label={t("settings.readingOverview.booksReading")} accent="text-warning" />
        <StatTile value={user.series_in_progress} label={t("settings.readingOverview.seriesInProgress")} accent="text-primary" />
      </div>

      {user.currently_reading.length > 0 && (
        <section>
          <SectionTitle count={user.currently_reading.length}>{t("settings.readingOverview.currentlyReading")}</SectionTitle>
          <div className="flex gap-3 sm:gap-4 overflow-x-auto pb-2 snap-x scrollbar-thin stagger-deal">
            {user.currently_reading.map((book) => (
              <CurrentlyReadingCard key={book.book_id} book={book} />
            ))}
          </div>
        </section>
      )}

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <section className="min-w-0">
          <SectionTitle
            count={seriesProgress.length}
            action={<SeriesViewToggle view={seriesView} onChange={setSeriesView} />}
          >
            {t("settings.readingOverview.seriesStatuses")}
          </SectionTitle>
          <SeriesList series={seriesProgress} userId={user.user_id} view={seriesView} />
        </section>

        {recentlyRead.length > 0 && (
          <section className="min-w-0">
            <SectionTitle>{t("settings.readingOverview.recentlyRead")}</SectionTitle>
            <div className="space-y-0.5">
              {recentlyRead.map((book) => (
                <RecentlyReadRow key={book.book_id} book={book} />
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function UserPicker({
  users,
  selectedId,
  onSelect,
}: {
  users: UserReadingOverviewDto[];
  selectedId: string;
  onSelect: (id: string) => void;
}) {
  const { t } = useTranslation();

  return (
    <div role="tablist" className="flex gap-2 overflow-x-auto pb-1 -mx-4 px-4 sm:mx-0 sm:px-0">
      {users.map((user) => {
        const active = user.user_id === selectedId;
        return (
          <button
            key={user.user_id}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onSelect(user.user_id)}
            className={`flex items-center gap-2.5 shrink-0 rounded-full border pl-1 pr-4 py-1 text-left transition-colors ${
              active
                ? "border-primary/50 bg-primary/10"
                : "border-border bg-card hover:bg-muted/50"
            }`}
          >
            <span
              className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold ${
                active ? "bg-primary text-primary-foreground" : "bg-muted text-muted-foreground"
              }`}
            >
              {user.username.charAt(0).toUpperCase()}
            </span>
            <span className="leading-tight">
              <span className="block text-sm font-medium text-foreground">{user.username}</span>
              <span className="block text-2xs text-muted-foreground">
                {user.last_read_at
                  ? `${t("settings.readingOverview.lastReadAt")} ${user.last_read_at}`
                  : `${user.books_read} ${t("settings.readingOverview.booksRead")}`}
              </span>
            </span>
          </button>
        );
      })}
    </div>
  );
}

export function ReadingOverview() {
  const { t } = useTranslation();
  const [overview, setOverview] = useState<UserReadingOverviewDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);

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

  const selected = overview.find((user) => user.user_id === selectedId) ?? overview[0];

  return (
    <div className="space-y-4">
      {overview.length > 1 && (
        <UserPicker users={overview} selectedId={selected.user_id} onSelect={setSelectedId} />
      )}
      <Card hover={false} className="p-4 sm:p-6">
        {overview.length === 1 && (
          <p className="font-semibold text-foreground mb-4">{selected.username}</p>
        )}
        <UserOverview key={selected.user_id} user={selected} />
      </Card>
    </div>
  );
}

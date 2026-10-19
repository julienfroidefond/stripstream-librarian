"use client";

import { memo, type ReactNode } from "react";
import Link from "next/link";

import { BookDto, ReadingStatus } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { useTranslation } from "@/lib/i18n/context";
import { JacketGrid } from "./motion/JacketGrid";
import { BookCardActions } from "./CardActions";
import {
  CoverBadge,
  CoverCard,
  CoverImage,
  CoverProgress,
  CoverRow,
  CoverScrim,
  type CoverBadgeTone,
} from "./ui/CoverCard";

/** Couleurs de statut reprises du lecteur : neutre, en cours, terminé. */
const readingStatusTones: Record<ReadingStatus, CoverBadgeTone | null> = {
  unread: null,
  reading: "info",
  read: "success",
};

interface BookCardProps {
  book: BookDto & { coverUrl?: string };
  readingStatus?: ReadingStatus;
  hasActiveUser?: boolean;
}

/** Libellé de tome, aligné sur les conventions du lecteur. */
function volumeLabel(book: BookDto): string | null {
  if (book.volume_type === "hs") return `HS${book.volume != null ? ` #${book.volume}` : ""}`;
  if (book.volume_type === "integral") return `INT${book.volume != null ? ` #${book.volume}` : ""}`;
  if (book.volume_type === "oneshot") return "One-shot";
  if (book.volume != null) return `#${book.volume}`;
  return null;
}

export const BookCard = memo(function BookCard({
  book,
  readingStatus,
  compact,
  hasActiveUser = true,
}: BookCardProps & { compact?: boolean }) {
  const { t } = useTranslation();
  const coverUrl = book.coverUrl || `/api/books/${book.id}/thumbnail`;
  const status = readingStatus ?? book.reading_status;
  const statusTone = status ? readingStatusTones[status] : null;
  const statusLabels: Record<ReadingStatus, string> = {
    unread: t("status.unread"),
    reading: t("status.reading"),
    read: t("status.read"),
  };

  const format = book.format ?? book.kind;
  const volume = volumeLabel(book);
  const isRead = status === "read";
  const progressLabel = t("books.readingProgress", { title: book.title });

  return (
    <CoverCard>
      <Link
        href={`/books/${book.id}`}
        aria-label={book.title}
        className="absolute inset-0 z-10 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      />

      <CoverImage
        src={coverUrl}
        alt={t("books.coverOf", { name: book.title })}
        dimmed={isRead}
      />

      <div className="pointer-events-none absolute inset-x-0 top-0 z-20 flex items-start justify-between gap-1 p-2">
        {format && <CoverBadge>{format}</CoverBadge>}
        {statusTone && status && (
          <CoverBadge tone={statusTone} className="ml-auto">
            {statusLabels[status]}
          </CoverBadge>
        )}
      </div>

      {book.reading_current_page != null && book.page_count != null && (
        <CoverProgress
          value={book.reading_current_page}
          max={book.page_count}
          label={progressLabel}
        />
      )}

      <CoverScrim reserveActions>
        <h3
          className={`font-medium text-white line-clamp-2 ${compact ? "text-xs" : "text-sm"}`}
          title={book.title}
        >
          {book.title}
        </h3>

        {book.author && (
          <p className="mt-1 text-2xs text-white/75 line-clamp-1">{book.author}</p>
        )}

        <div className="mt-1.5 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-2xs">
          {book.series && (
            <span className="min-w-0 text-white/70 line-clamp-1">{book.series}</span>
          )}
          {volume && (
            <span className="shrink-0 font-semibold text-primary">{volume}</span>
          )}
          {book.language && (
            <span className="shrink-0 rounded-full bg-white/15 px-1.5 py-0.5 text-3xs font-medium uppercase tracking-wide text-white/90">
              {book.language}
            </span>
          )}
        </div>
      </CoverScrim>

      {/* Always visible so the shortcuts stay reachable on touch. */}
      <div className="absolute bottom-2 right-2 z-30">
        <BookCardActions book={book} hasActiveUser={hasActiveUser} variant="cover" />
      </div>
    </CoverCard>
  );
});

interface BooksGridProps {
  books: (BookDto & { coverUrl?: string })[];
  compact?: boolean;
  hasActiveUser?: boolean;
}

export function BooksGrid({ books, compact, hasActiveUser = true }: BooksGridProps) {
  return (
    <JacketGrid className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
      {books.map((book) => (
        <BookCard key={book.id} book={book} compact={compact} hasActiveUser={hasActiveUser} />
      ))}
    </JacketGrid>
  );
}

export interface MissingBook {
  title: string | null;
  volume_number: number | null;
  cover_url: string | null;
  href?: string;
}

function MissingBookCard({ book }: { book: MissingBook }) {
  const { t } = useTranslation();
  const title = book.title || t("books.unknown");

  return (
    <div className="group relative aspect-[2/3] overflow-hidden rounded-xl border border-dashed border-warning/40 bg-muted opacity-60 transition-opacity duration-200 hover:opacity-100">
      {book.href && (
        <Link
          href={book.href as `/books/missing?${string}`}
          aria-label={title}
          className="absolute inset-0 z-10 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
        />
      )}

      <CoverImage
        src={book.cover_url ?? ""}
        alt={title}
        dimmed
        fallback="book"
      />

      <div className="pointer-events-none absolute inset-x-0 top-0 z-20 flex items-start justify-end p-2">
        <CoverBadge tone="warning">{t("status.missing")}</CoverBadge>
      </div>

      <CoverScrim>
        <h3 className="text-xs font-medium text-warning line-clamp-2" title={book.title || undefined}>
          {title}
        </h3>
        {book.volume_number != null && (
          <span className="mt-1 block text-2xs font-semibold text-warning/80">
            #{book.volume_number}
          </span>
        )}
      </CoverScrim>
    </div>
  );
}

type MergedBookItem =
  | { kind: "owned"; book: BookDto & { coverUrl?: string } }
  | { kind: "missing"; book: MissingBook };

/**
 * Merge owned and missing books, sorted by volume number.
 *
 * Filter out missing entries whose volume number is already covered by an owned
 * book (handles the case where the link's `external_book_metadata` wasn't
 * rematched after the local book was scanned).
 */
function mergeBooksWithMissing(
  books: (BookDto & { coverUrl?: string })[],
  missingBooks: MissingBook[],
): MergedBookItem[] {
  const ownedVolumes = new Set(
    books.map((b) => b.volume).filter((v): v is number => v != null)
  );
  const ownedTitleVolumes = new Set<number>();
  for (const b of books) {
    // Extract a trailing volume number from the title as a fallback when
    // book.volume is NULL in DB (e.g., older imports before parser fix).
    const m = b.title?.match(/(?:t|tome|vol\.?|volume)\s*0*(\d{1,3})\b/i);
    if (m) ownedTitleVolumes.add(parseInt(m[1], 10));
    const trailing = b.title?.match(/\s0*(\d{1,3})$/);
    if (trailing) ownedTitleVolumes.add(parseInt(trailing[1], 10));
  }

  const filteredMissing = missingBooks.filter(
    (m) =>
      m.volume_number == null ||
      (!ownedVolumes.has(m.volume_number) && !ownedTitleVolumes.has(m.volume_number))
  );

  return [
    ...books.map((b) => ({ kind: "owned" as const, book: b })),
    ...filteredMissing.map((b) => ({ kind: "missing" as const, book: b })),
  ].sort((a, b) => {
    const va = a.kind === "owned" ? a.book.volume : a.book.volume_number;
    const vb = b.kind === "owned" ? b.book.volume : b.book.volume_number;
    if (va == null && vb == null) return 0;
    if (va == null) return 1;
    if (vb == null) return -1;
    return va - vb;
  });
}

export function BooksGridWithMissing({
  books,
  missingBooks,
  showMissing,
  compact,
  hasActiveUser = true,
}: {
  books: (BookDto & { coverUrl?: string })[];
  missingBooks: MissingBook[];
  showMissing: boolean;
  compact?: boolean;
  hasActiveUser?: boolean;
}) {
  if (!showMissing) {
    return <BooksGrid books={books} compact={compact} hasActiveUser={hasActiveUser} />;
  }

  const merged = mergeBooksWithMissing(books, missingBooks);

  return (
    <JacketGrid className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
      {merged.map((item, idx) =>
        item.kind === "owned" ? (
          <BookCard key={item.book.id} book={item.book} compact={compact} hasActiveUser={hasActiveUser} />
        ) : (
          <MissingBookCard
            key={item.book.volume_number != null ? `missing-vol-${item.book.volume_number}` : `missing-idx-${idx}`}
            book={item.book}
          />
        )
      )}
    </JacketGrid>
  );
}

/** Row equivalent of `MissingBookCard`, used by the list view of a series. */
function MissingBookRow({ book }: { book: MissingBook }) {
  const { t } = useTranslation();
  const title = book.title || t("books.unknown");

  return (
    <div className="group relative flex items-center gap-3 overflow-hidden rounded-xl border border-dashed border-warning/40 bg-card p-2 transition-colors duration-200 hover:bg-warning/5 sm:gap-4 sm:p-2.5">
      {book.href && (
        <Link
          href={book.href as `/books/missing?${string}`}
          aria-label={title}
          className="absolute inset-0 z-10 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
        />
      )}

      <div className="relative aspect-[2/3] w-12 shrink-0 overflow-hidden rounded-lg bg-muted sm:w-14">
        <CoverImage src={book.cover_url ?? ""} alt={title} dimmed fallback="book" />
      </div>

      <div className="flex min-w-0 flex-1 flex-col justify-center gap-1">
        <div className="flex min-w-0 items-center gap-2">
          <h3 className="min-w-0 flex-1 truncate text-sm font-semibold text-warning" title={book.title || undefined}>
            {title}
          </h3>
          {book.volume_number != null && (
            <span className="shrink-0 rounded-full bg-warning/15 px-1.5 py-0.5 text-2xs font-semibold text-warning">
              #{book.volume_number}
            </span>
          )}
          <CoverBadge tone="warning" className="z-20 shrink-0">
            {t("status.missing")}
          </CoverBadge>
        </div>
        <p className="text-xs text-muted-foreground">{t("books.missing")}</p>
      </div>
    </div>
  );
}

/**
 * List view of the same books data. Cover-first rows sharing the `CoverRow`
 * chrome, metadata inline, and the mark-read action always visible so it stays
 * reachable on touch.
 */
function BookListRow({ book, hasActiveUser }: { book: BookDto & { coverUrl?: string }; hasActiveUser: boolean }) {
  const { t, locale } = useTranslation();
  const coverUrl = book.coverUrl || `/api/books/${book.id}/thumbnail`;
  const status = book.reading_status;
  const isRead = status === "read";
  const volume = volumeLabel(book);
  const format = book.format ?? book.kind;
  const author = book.author || (book.authors.length > 0 ? book.authors.join(", ") : null);

  const statusLabels: Record<ReadingStatus, string> = {
    unread: t("status.unread"),
    reading: t("status.reading"),
    read: t("status.read"),
  };
  const statusTones: Record<ReadingStatus, CoverBadgeTone> = {
    unread: "muted",
    reading: "info",
    read: "success",
  };

  const progressLabel = t("books.readingProgress", { title: book.title });
  const hasProgress =
    book.reading_current_page != null && book.page_count != null && book.page_count > 0 && !isRead;
  const percent = hasProgress
    ? Math.min(100, Math.max(0, Math.round((book.reading_current_page! / book.page_count!) * 100)))
    : 0;

  return (
    <CoverRow>
      <Link
        href={`/books/${book.id}`}
        aria-label={book.title}
        className="absolute inset-0 z-10 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
      />

      <div className="relative aspect-[2/3] w-12 shrink-0 overflow-hidden rounded-lg bg-muted sm:w-14">
        <CoverImage
          src={coverUrl}
          alt={t("books.coverOf", { name: book.title })}
          dimmed={isRead}
          sizes="64px"
        />
      </div>

      <div className="flex min-w-0 flex-1 flex-col justify-center gap-1.5">
        <div className="flex min-w-0 items-center gap-2">
          <h3
            className="min-w-0 flex-1 truncate text-sm font-semibold text-foreground transition-colors group-hover:text-primary"
            title={book.title}
          >
            {book.title}
          </h3>
          {volume && (
            <span className="shrink-0 rounded-full bg-muted px-1.5 py-0.5 text-2xs font-semibold text-muted-foreground">
              {volume}
            </span>
          )}
          <CoverBadge tone={statusTones[status]} className="z-20 shrink-0">
            {statusLabels[status]}
          </CoverBadge>
        </div>

        {book.summary && (
          <p className="line-clamp-2 text-xs leading-relaxed text-muted-foreground/80">
            {book.summary}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
          {author && <span className="truncate">{author}</span>}
          {book.series && <span className="truncate">{book.series}</span>}
          {book.language && <span className="uppercase">{book.language}</span>}
          {format && <span className="uppercase">{format}</span>}
          {book.page_count != null && (
            <span>
              {t("books.pagesCount", { count: String(book.page_count), plural: book.page_count !== 1 ? "s" : "" })}
            </span>
          )}
          {book.file_size != null && (
            <span title={t("books.fileSize")}>{formatBytes(book.file_size, locale)}</span>
          )}
          {book.publish_date && (
            <span title={t("field.publish_date")}>{book.publish_date}</span>
          )}
        </div>

        {hasProgress && (
          <span
            className="flex items-center gap-2"
            role="progressbar"
            aria-valuenow={percent}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={progressLabel}
          >
            <span className="h-1 w-full max-w-40 overflow-hidden rounded-full bg-muted">
              <span className="block h-full rounded-full bg-primary" style={{ width: `${percent}%` }} />
            </span>
            <span className="shrink-0 tabular-nums text-2xs text-muted-foreground">
              {book.reading_current_page} / {book.page_count}
            </span>
          </span>
        )}
      </div>

      <div className="relative z-20 shrink-0 self-center">
        <BookCardActions book={book} hasActiveUser={hasActiveUser} variant="inline" />
      </div>
    </CoverRow>
  );
}

export function BooksList({
  books,
  hasActiveUser = true,
}: {
  books: (BookDto & { coverUrl?: string })[];
  hasActiveUser?: boolean;
}) {
  return (
    <div className="flex flex-col gap-2">
      {books.map((book) => (
        <BookListRow key={book.id} book={book} hasActiveUser={hasActiveUser} />
      ))}
    </div>
  );
}

/**
 * List equivalent of `BooksGridWithMissing`: same merged, volume-sorted list,
 * strong (owned) rows mixed with dashed missing-volume rows.
 */
export function BooksListWithMissing({
  books,
  missingBooks,
  showMissing,
  hasActiveUser = true,
}: {
  books: (BookDto & { coverUrl?: string })[];
  missingBooks: MissingBook[];
  showMissing: boolean;
  hasActiveUser?: boolean;
}) {
  if (!showMissing) {
    return <BooksList books={books} hasActiveUser={hasActiveUser} />;
  }

  const merged = mergeBooksWithMissing(books, missingBooks);

  return (
    <div className="flex flex-col gap-2">
      {merged.map((item, idx) =>
        item.kind === "owned" ? (
          <BookListRow key={item.book.id} book={item.book} hasActiveUser={hasActiveUser} />
        ) : (
          <MissingBookRow
            key={item.book.volume_number != null ? `missing-vol-${item.book.volume_number}` : `missing-idx-${idx}`}
            book={item.book}
          />
        )
      )}
    </div>
  );
}

interface EmptyStateProps {
  message: string;
  icon?: ReactNode;
}

export function EmptyState({ message, icon }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-16 h-16 mb-4 text-muted-foreground/30">
        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {icon ?? <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />}
        </svg>
      </div>
      <p className="text-muted-foreground text-lg">{message}</p>
    </div>
  );
}

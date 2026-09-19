"use client";

import { memo, useState } from "react";
import Image from "next/image";
import Link from "next/link";
import { BookDto, ReadingStatus } from "../../lib/api";
import { useTranslation } from "../../lib/i18n/context";
import { MarkBookReadButton } from "./MarkBookReadButton";

const readingStatusOverlayClasses: Record<ReadingStatus, string | null> = {
  unread: null,
  reading: "bg-amber-500/90 text-white",
  read: "bg-green-600/90 text-white",
};

interface BookCardProps {
  book: BookDto & { coverUrl?: string };
  readingStatus?: ReadingStatus;
  hasActiveUser?: boolean;
}

const BookImage = memo(function BookImage({ src, alt, dimmed }: { src: string; alt: string; dimmed?: boolean }) {
  const [isLoaded, setIsLoaded] = useState(false);
  const [hasError, setHasError] = useState(false);

  if (hasError) {
    return (
      <div className="relative aspect-[2/3] overflow-hidden bg-muted flex items-center justify-center">
        <svg className="w-10 h-10 text-muted-foreground/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
        </svg>
      </div>
    );
  }

  return (
    <div className="relative aspect-[2/3] overflow-hidden bg-muted">
      {/* Skeleton */}
      <div
        className={`absolute inset-0 bg-muted/50 animate-pulse transition-opacity duration-300 ${
          isLoaded ? 'opacity-0 pointer-events-none' : 'opacity-100'
        }`}
      />

      {/* Image */}
      <Image
        src={src}
        alt={alt}
        fill
        className={`object-cover group-hover:scale-105 transition-transform duration-300 ${
          isLoaded ? (dimmed ? 'opacity-40' : 'opacity-100') : 'opacity-0'
        }`}
        sizes="(max-width: 640px) 50vw, (max-width: 768px) 33vw, (max-width: 1024px) 25vw, 16vw"
        onLoad={() => setIsLoaded(true)}
        onError={() => setHasError(true)}
      />
    </div>
  );
});

export const BookCard = memo(function BookCard({ book, readingStatus, compact, hasActiveUser = true }: BookCardProps & { compact?: boolean }) {
  const { t } = useTranslation();
  const coverUrl = book.coverUrl || `/api/books/${book.id}/thumbnail`;
  const status = readingStatus ?? book.reading_status;
  const overlayClass = status ? readingStatusOverlayClasses[status] : null;
  const statusLabels: Record<ReadingStatus, string> = {
    unread: t("status.unread"),
    reading: t("status.reading"),
    read: t("status.read"),
  };

  const isRead = status === "read";

  if (compact) {
    return (
      <div className="group bg-card rounded-xl border border-border/60 shadow-sm hover:shadow-md hover:-translate-y-1 transition-all duration-200 overflow-hidden">
        <Link href={`/books/${book.id}`} className="block">
          <div className="relative">
            <BookImage
              src={coverUrl}
              alt={t("books.coverOf", { name: book.title })}
              dimmed={isRead}
            />
            {overlayClass && status && (
              <span className={`absolute top-1.5 right-1.5 px-1.5 py-0.5 rounded-full text-[10px] font-bold tracking-wide ${overlayClass}`}>
                {statusLabels[status]}
              </span>
            )}
          </div>
        </Link>
        <div className="px-2 py-1.5">
          <Link href={`/books/${book.id}`}>
            <h3 className="font-medium text-foreground truncate text-xs hover:text-primary transition-colors" title={book.title}>
              {book.title}
            </h3>
          </Link>
          <div className="flex items-center justify-between mt-0.5">
            <div className="flex items-center gap-1">
              {(book.format ?? book.kind) && (
                <span className={`
                  px-1 py-0.5 text-[9px] font-bold uppercase rounded-full
                  ${(book.format ?? book.kind) === 'cbz' ? 'bg-success/10 text-success' : ''}
                  ${(book.format ?? book.kind) === 'cbr' ? 'bg-warning/10 text-warning' : ''}
                  ${(book.format ?? book.kind) === 'pdf' ? 'bg-destructive/10 text-destructive' : ''}
                  ${(book.format ?? book.kind) === 'epub' ? 'bg-info/10 text-info' : ''}
                `}>
                  {book.format ?? book.kind}
                </span>
              )}
              {book.volume_type === "hs" ? (
                <span className="text-[10px] font-medium text-orange-500">HS{book.volume != null ? ` #${book.volume}` : ""}</span>
              ) : book.volume_type === "integral" ? (
                <span className="text-[10px] font-medium text-blue-500">INT{book.volume != null ? ` #${book.volume}` : ""}</span>
              ) : book.volume_type === "oneshot" ? (
                <span className="text-[10px] font-medium text-purple-500">One-shot</span>
              ) : book.volume != null ? (
                <span className="text-[10px] text-muted-foreground">#{book.volume}</span>
              ) : null}
            </div>
            {hasActiveUser && (
              <MarkBookReadButton
                bookId={book.id}
                currentStatus={status ?? "unread"}
                compact
              />
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <Link
      href={`/books/${book.id}`}
      className="group block bg-card rounded-xl border border-border/60 shadow-sm hover:shadow-md hover:-translate-y-1 transition-all duration-200 overflow-hidden"
    >
      <div className="relative">
        <BookImage
          src={coverUrl}
          alt={t("books.coverOf", { name: book.title })}
          dimmed={isRead}
        />
        {overlayClass && status && (
          <span className={`absolute top-1.5 right-1.5 px-1.5 py-0.5 rounded-full text-[10px] font-bold tracking-wide ${overlayClass}`}>
            {statusLabels[status]}
          </span>
        )}
      </div>

      <div className="p-4">
        <h3
          className="font-semibold text-foreground line-clamp-2 min-h-[2.5rem]"
          title={book.title}
        >
          {book.title}
        </h3>

        {book.author && (
          <p className="text-sm text-muted-foreground mb-1 truncate">{book.author}</p>
        )}

        {book.series && (
          <p className="text-xs text-muted-foreground/80 truncate mb-2">
            {book.series}
            {book.volume_type === "hs" ? (
              <span className="text-orange-500 font-medium"> HS{book.volume != null ? ` #${book.volume}` : ""}</span>
            ) : book.volume_type === "integral" ? (
              <span className="text-blue-500 font-medium"> INT{book.volume != null ? ` #${book.volume}` : ""}</span>
            ) : book.volume_type === "oneshot" ? (
              <span className="text-purple-500 font-medium"> One-shot</span>
            ) : book.volume != null ? (
              <span className="text-primary font-medium"> #{book.volume}</span>
            ) : null}
          </p>
        )}

        <div className="flex items-center gap-2 mt-2">
          {(book.format ?? book.kind) && (
            <span className={`
              px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full
              ${(book.format ?? book.kind) === 'cbz' ? 'bg-success/10 text-success' : ''}
              ${(book.format ?? book.kind) === 'cbr' ? 'bg-warning/10 text-warning' : ''}
              ${(book.format ?? book.kind) === 'pdf' ? 'bg-destructive/10 text-destructive' : ''}
              ${(book.format ?? book.kind) === 'epub' ? 'bg-info/10 text-info' : ''}
            `}>
              {book.format ?? book.kind}
            </span>
          )}
          {book.language && (
            <span className="px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider rounded-full bg-primary/10 text-primary">
              {book.language}
            </span>
          )}
        </div>
      </div>
    </Link>
  );
});

interface BooksGridProps {
  books: (BookDto & { coverUrl?: string })[];
  compact?: boolean;
  hasActiveUser?: boolean;
}

export function BooksGrid({ books, compact, hasActiveUser = true }: BooksGridProps) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
      {books.map((book) => (
        <BookCard key={book.id} book={book} compact={compact} hasActiveUser={hasActiveUser} />
      ))}
    </div>
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

  const content = (
    <>
      <div className="relative aspect-[2/3] overflow-hidden bg-muted">
        {book.cover_url ? (
          /* eslint-disable-next-line @next/next/no-img-element */
          <img
            src={book.cover_url}
            alt={book.title || t("books.missing")}
            className="w-full h-full object-cover grayscale group-hover:grayscale-0 transition-all duration-200"
          />
        ) : (
          <div className="flex items-center justify-center h-full">
            <svg className="w-10 h-10 text-muted-foreground/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
            </svg>
          </div>
        )}
        <span className="absolute top-1.5 right-1.5 px-1.5 py-0.5 rounded-full text-[10px] font-bold tracking-wide bg-muted-foreground/80 text-white">
          {t("status.missing")}
        </span>
      </div>
      <div className="px-2 py-1.5">
        <h3 className="font-medium text-muted-foreground truncate text-xs" title={book.title || undefined}>
          {book.title || t("books.unknown")}
        </h3>
        {book.volume_number != null && (
          <span className="text-[10px] text-muted-foreground">#{book.volume_number}</span>
        )}
      </div>
    </>
  );

  const baseClass = "group bg-card rounded-xl border border-dashed border-border/60 shadow-sm overflow-hidden opacity-50";

  if (book.href) {
    return (
      <Link
        href={book.href as any}
        className={`${baseClass} hover:opacity-100 hover:shadow-md hover:-translate-y-1 transition-all duration-200`}
      >
        {content}
      </Link>
    );
  }

  return <div className={baseClass}>{content}</div>;
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

  // Merge owned and missing books, sorted by volume_number.
  // Filter out missing entries whose volume_number is already covered by an
  // owned book (handles the case where the link's external_book_metadata
  // wasn't rematched after the local book was scanned).
  type MergedItem =
    | { kind: "owned"; book: BookDto & { coverUrl?: string } }
    | { kind: "missing"; book: MissingBook };

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

  const merged: MergedItem[] = [
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

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
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
    </div>
  );
}

interface EmptyStateProps {
  message: string;
}

export function EmptyState({ message }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-16 h-16 mb-4 text-muted-foreground/30">
        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
        </svg>
      </div>
      <p className="text-muted-foreground text-lg">{message}</p>
    </div>
  );
}

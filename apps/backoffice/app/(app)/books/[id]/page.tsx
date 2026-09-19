import { fetchLibraries, getBookCoverUrl, BookDto, apiFetch, ReadingStatus, TelegramMonitorStatus } from "@/lib/api";
import { cookies } from "next/headers";
import { BookPreview } from "@/app/components/BookPreview";
import { BookActionsToolbar } from "@/app/components/BookActionsToolbar";
import { SafeHtml } from "@/app/components/SafeHtml";
import { getServerTranslations } from "@/lib/i18n/server";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";

export const dynamic = "force-dynamic";

const readingStatusClassNames: Record<ReadingStatus, string> = {
  unread: "bg-muted/60 text-muted-foreground border border-border",
  reading: "bg-amber-500/15 text-amber-600 dark:text-amber-400 border border-amber-500/30",
  read: "bg-green-500/15 text-green-600 dark:text-green-400 border border-green-500/30",
};

async function fetchBook(bookId: string): Promise<BookDto | null> {
  try {
    return await apiFetch<BookDto>(`/books/${bookId}`);
  } catch {
    return null;
  }
}

export default async function BookDetailPage({
  params
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const cookieStore = await cookies();
  const hasActiveUser = !!cookieStore.get("as_user_id")?.value;
  const [book, libraries, telegramStatus] = await Promise.all([
    fetchBook(id),
    fetchLibraries().catch(() => [] as { id: string; name: string }[]),
    apiFetch<TelegramMonitorStatus>("/telegram-monitor/status", { next: { revalidate: 60 } }).catch(() => null),
  ]);
  const telegramEnabled = !!(telegramStatus?.configured && telegramStatus?.authorized);

  if (!book) {
    notFound();
  }

  const { t, locale } = await getServerTranslations();

  const library = libraries.find(l => l.id === book.library_id);
  const formatBadge = (book.format ?? book.kind).toUpperCase();
  const formatColor =
    formatBadge === "CBZ" ? "bg-success/10 text-success border-success/30" :
    formatBadge === "CBR" ? "bg-warning/10 text-warning border-warning/30" :
    formatBadge === "PDF" ? "bg-destructive/10 text-destructive border-destructive/30" :
    "bg-muted/50 text-muted-foreground border-border";
  const statusLabel = t(`status.${book.reading_status}` as "status.unread" | "status.reading" | "status.read");
  const statusClassName = readingStatusClassNames[book.reading_status];

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm">
        <Link href="/libraries" className="text-muted-foreground hover:text-primary transition-colors">
          {t("bookDetail.libraries")}
        </Link>
        <span className="text-muted-foreground">/</span>
        {library && (
          <>
            <Link
              href={`/libraries/${book.library_id}/series`}
              className="text-muted-foreground hover:text-primary transition-colors"
            >
              {library.name}
            </Link>
            <span className="text-muted-foreground">/</span>
          </>
        )}
        {book.series && book.series_id && (
          <>
            <Link
              href={`/series/${book.series_id}`}
              className="text-muted-foreground hover:text-primary transition-colors"
            >
              {book.series}
            </Link>
            <span className="text-muted-foreground">/</span>
          </>
        )}
        <span className="text-foreground font-medium truncate">{book.title}</span>
      </div>

      {/* Hero */}
      <div className="flex flex-col sm:flex-row gap-6">
        {/* Cover */}
        <div className="flex-shrink-0">
          <div className="w-40 aspect-[2/3] relative rounded-xl overflow-hidden shadow-card border border-border">
            <Image
              src={getBookCoverUrl(book.id, book.updated_at)}
              alt={t("bookDetail.coverOf", { title: book.title })}
              fill
              className="object-cover"
              sizes="160px"
              loading="lazy"
            />
          </div>
        </div>

        {/* Info */}
        <div className="flex-1 space-y-4">
          {/* Title */}
          <h1 className="text-3xl font-bold text-foreground">{book.title}</h1>

          {/* Author + Series + Volume + Reading status badges */}
          <div className="flex flex-wrap items-center gap-3">
            {book.author && (
              <p className="text-base text-muted-foreground">{book.author}</p>
            )}
            {book.series && book.series_id && (
              <Link
                href={`/series/${book.series_id}`}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs border border-primary/30 font-medium"
              >
                {book.series}
                {book.volume != null && <span className="font-semibold">Vol. {book.volume}</span>}
              </Link>
            )}
            <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${statusClassName}`}>
              {statusLabel}
              {book.reading_status === "reading" && book.reading_current_page != null && ` · p. ${book.reading_current_page}`}
            </span>
            {book.reading_last_read_at && (
              <span className="text-xs text-muted-foreground">
                {new Date(book.reading_last_read_at).toLocaleDateString(locale)}
              </span>
            )}
          </div>

          {/* Metadata stats */}
          <div className="flex flex-wrap items-center gap-4 text-sm">
            <span className={`inline-flex px-2.5 py-0.5 rounded-full text-xs font-semibold border ${formatColor}`}>
              {formatBadge}
            </span>
            {book.page_count && (
              <span className="text-muted-foreground"><span className="font-semibold text-foreground">{book.page_count}</span> {t("dashboard.pages").toLowerCase()}</span>
            )}
            {book.language && (
              <span className="text-muted-foreground">{book.language.toUpperCase()}</span>
            )}
            {book.isbn && (
              <span className="text-muted-foreground font-mono">ISBN {book.isbn}</span>
            )}
            {book.publish_date && (
              <span className="text-muted-foreground">{book.publish_date}</span>
            )}
          </div>

          {/* Description */}
          {book.summary && (
            <SafeHtml html={book.summary} className="text-sm text-muted-foreground leading-relaxed" />
          )}

          {/* Action buttons toolbar */}
          <BookActionsToolbar book={book} hasActiveUser={hasActiveUser} telegramEnabled={telegramEnabled} />
        </div>
      </div>

      {/* Technical info (collapsible) */}
      <details className="group">
        <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground transition-colors select-none flex items-center gap-1.5">
          <svg className="w-3.5 h-3.5 transition-transform group-open:rotate-90" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
          {t("bookDetail.technicalInfo")}
        </summary>
        <div className="mt-3 p-4 rounded-lg bg-muted/30 border border-border/50 space-y-2 text-xs">
          {book.file_path && (
            <div className="flex flex-col gap-0.5">
              <span className="text-muted-foreground">{t("bookDetail.file")}</span>
              <code className="font-mono text-foreground break-all">{book.file_path}</code>
            </div>
          )}
          {book.file_format && (
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">{t("bookDetail.fileFormat")}</span>
              <span className="text-foreground">{book.file_format.toUpperCase()}</span>
            </div>
          )}
          {book.file_parse_status && (
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">{t("bookDetail.parsing")}</span>
              <span className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${
                book.file_parse_status === "success" ? "bg-success/10 text-success" :
                book.file_parse_status === "failed" ? "bg-destructive/10 text-destructive" :
                "bg-muted/50 text-muted-foreground"
              }`}>
                {book.file_parse_status}
              </span>
            </div>
          )}
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground">Book ID</span>
            <code className="font-mono text-foreground">{book.id}</code>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground">Library ID</span>
            <code className="font-mono text-foreground">{book.library_id}</code>
          </div>
          {book.updated_at && (
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">{t("bookDetail.updatedAt")}</span>
              <span className="text-foreground">{new Date(book.updated_at).toLocaleString(locale)}</span>
            </div>
          )}
        </div>
      </details>

      {/* Book Preview */}
      {book.page_count && book.page_count > 0 && (
        <BookPreview key={book.id} bookId={book.id} pageCount={book.page_count} version={book.updated_at} />
      )}
    </div>
  );
}

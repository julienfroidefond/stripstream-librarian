import { fetchLibraries, getBookCoverUrl, BookDto, apiFetch, ReadingStatus } from "@/lib/api";
import { BookPreview } from "@/app/components/BookPreview";
import { ConvertButton } from "@/app/components/ConvertButton";
import { MarkBookReadButton } from "@/app/components/MarkBookReadButton";
import nextDynamic from "next/dynamic";
import { SafeHtml } from "@/app/components/SafeHtml";
import { getServerTranslations } from "@/lib/i18n/server";
import Image from "next/image";
import Link from "next/link";

const EditBookForm = nextDynamic(
  () => import("@/app/components/EditBookForm").then(m => m.EditBookForm)
);
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
  const [book, libraries] = await Promise.all([
    fetchBook(id),
    fetchLibraries().catch(() => [] as { id: string; name: string }[])
  ]);

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
        {book.series && (
          <>
            <Link
              href={`/libraries/${book.library_id}/series/${encodeURIComponent(book.series)}`}
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
          <div className="w-48 aspect-[2/3] relative rounded-xl overflow-hidden shadow-card border border-border">
            <Image
              src={getBookCoverUrl(book.id)}
              alt={t("bookDetail.coverOf", { title: book.title })}
              fill
              className="object-cover"
              sizes="192px"
              loading="lazy"
            />
          </div>
        </div>

        {/* Info */}
        <div className="flex-1 space-y-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-3xl font-bold text-foreground">{book.title}</h1>
              {book.author && (
                <p className="text-base text-muted-foreground mt-1">{book.author}</p>
              )}
            </div>
            <EditBookForm book={book} />
          </div>

          {/* Series + Volume link */}
          {book.series && (
            <div className="flex items-center gap-2 text-sm">
              <Link
                href={`/libraries/${book.library_id}/series/${encodeURIComponent(book.series)}`}
                className="text-primary hover:text-primary/80 transition-colors font-medium"
              >
                {book.series}
              </Link>
              {book.volume != null && (
                <span className="px-2 py-0.5 bg-primary/10 text-primary rounded-md text-xs font-semibold">
                  Vol. {book.volume}
                </span>
              )}
            </div>
          )}

          {/* Reading status + actions */}
          <div className="flex flex-wrap items-center gap-3">
            <span className={`inline-flex items-center px-2.5 py-1 rounded-full text-xs font-semibold ${statusClassName}`}>
              {statusLabel}
              {book.reading_status === "reading" && book.reading_current_page != null && ` · p. ${book.reading_current_page}`}
            </span>
            {book.reading_last_read_at && (
              <span className="text-xs text-muted-foreground">
                {new Date(book.reading_last_read_at).toLocaleDateString(locale)}
              </span>
            )}
            <MarkBookReadButton bookId={book.id} currentStatus={book.reading_status} />
            {book.file_format === "cbr" && <ConvertButton bookId={book.id} />}
          </div>

          {/* Metadata pills */}
          <div className="flex flex-wrap items-center gap-2">
            <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-semibold border ${formatColor}`}>
              {formatBadge}
            </span>
            {book.page_count && (
              <span className="inline-flex px-2.5 py-1 rounded-full text-xs font-medium bg-muted/50 text-muted-foreground border border-border">
                {book.page_count} {t("dashboard.pages").toLowerCase()}
              </span>
            )}
            {book.language && (
              <span className="inline-flex px-2.5 py-1 rounded-full text-xs font-medium bg-muted/50 text-muted-foreground border border-border">
                {book.language.toUpperCase()}
              </span>
            )}
            {book.isbn && (
              <span className="inline-flex px-2.5 py-1 rounded-full text-xs font-mono font-medium bg-muted/50 text-muted-foreground border border-border">
                ISBN {book.isbn}
              </span>
            )}
            {book.publish_date && (
              <span className="inline-flex px-2.5 py-1 rounded-full text-xs font-medium bg-muted/50 text-muted-foreground border border-border">
                {book.publish_date}
              </span>
            )}
          </div>

          {/* Description */}
          {book.summary && (
            <SafeHtml html={book.summary} className="text-sm text-muted-foreground leading-relaxed" />
          )}
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
        <BookPreview bookId={book.id} pageCount={book.page_count} />
      )}
    </div>
  );
}

import { fetchBooks, searchBooks, fetchLibraries, BookDto, LibraryDto, SeriesHitDto, getBookCoverUrl } from "../../lib/api";
import { BooksGrid, EmptyState } from "../components/BookCard";
import { LiveSearchForm } from "../components/LiveSearchForm";
import { Card, CardContent, OffsetPagination } from "../components/ui";
import Link from "next/link";
import Image from "next/image";
import { getServerTranslations } from "../../lib/i18n/server";

export const dynamic = "force-dynamic";

export default async function BooksPage({
  searchParams
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const searchParamsAwaited = await searchParams;
  const libraryId = typeof searchParamsAwaited.library === "string" ? searchParamsAwaited.library : undefined;
  const searchQuery = typeof searchParamsAwaited.q === "string" ? searchParamsAwaited.q : "";
  const readingStatus = typeof searchParamsAwaited.status === "string" ? searchParamsAwaited.status : undefined;
  const format = typeof searchParamsAwaited.format === "string" ? searchParamsAwaited.format : undefined;
  const metadataProvider = typeof searchParamsAwaited.metadata === "string" ? searchParamsAwaited.metadata : undefined;
  const sort = typeof searchParamsAwaited.sort === "string" ? searchParamsAwaited.sort : undefined;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const [libraries] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[])
  ]);

  let books: BookDto[] = [];
  let total = 0;
  let searchResults: BookDto[] | null = null;
  let seriesHits: SeriesHitDto[] = [];
  let totalHits: number | null = null;

  if (searchQuery) {
    const searchResponse = await searchBooks(searchQuery, libraryId, limit).catch(() => null);
    if (searchResponse) {
      seriesHits = searchResponse.series_hits ?? [];
      searchResults = searchResponse.hits.map(hit => ({
        id: hit.id,
        library_id: hit.library_id,
        kind: hit.kind,
        title: hit.title,
        author: hit.authors?.[0] ?? null,
        authors: hit.authors ?? [],
        series: hit.series,
        volume: hit.volume,
        language: hit.language,
        page_count: null,
        format: null,
        file_path: null,
        file_format: null,
        file_parse_status: null,
        updated_at: "",
        reading_status: "unread" as const,
        reading_current_page: null,
        reading_last_read_at: null,
        summary: null,
        isbn: null,
        publish_date: null,
      }));
      totalHits = searchResponse.estimated_total_hits;
    }
  } else {
    const booksPage = await fetchBooks(libraryId, undefined, page, limit, readingStatus, sort, undefined, format, metadataProvider).catch(() => ({
      items: [] as BookDto[],
      total: 0,
      page: 1,
      limit,
    }));
    books = booksPage.items;
    total = booksPage.total;
  }

  const displayBooks = (searchResults || books).map(book => ({
    ...book,
    coverUrl: getBookCoverUrl(book.id)
  }));

  const totalPages = Math.ceil(total / limit);

  const libraryOptions = [
    { value: "", label: t("books.allLibraries") },
    ...libraries.map((lib) => ({ value: lib.id, label: lib.name })),
  ];

  const statusOptions = [
    { value: "", label: t("common.all") },
    { value: "unread", label: t("status.unread") },
    { value: "reading", label: t("status.reading") },
    { value: "read", label: t("status.read") },
  ];

  const formatOptions = [
    { value: "", label: t("books.allFormats") },
    { value: "cbz", label: "CBZ" },
    { value: "cbr", label: "CBR" },
    { value: "pdf", label: "PDF" },
    { value: "epub", label: "EPUB" },
  ];

  const metadataOptions = [
    { value: "", label: t("series.metadataAll") },
    { value: "linked", label: t("series.metadataLinked") },
    { value: "unlinked", label: t("series.metadataUnlinked") },
  ];

  const sortOptions = [
    { value: "", label: t("books.sortTitle") },
    { value: "latest", label: t("books.sortLatest") },
  ];

  const hasFilters = searchQuery || libraryId || readingStatus || format || metadataProvider || sort;

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
          {t("books.title")}
        </h1>
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <LiveSearchForm
            basePath="/books"
            fields={[
              { name: "q", type: "text", label: t("common.search"), placeholder: t("books.searchPlaceholder") },
              { name: "library", type: "select", label: t("books.library"), options: libraryOptions },
              { name: "status", type: "select", label: t("books.status"), options: statusOptions },
              { name: "format", type: "select", label: t("books.format"), options: formatOptions },
              { name: "metadata", type: "select", label: t("series.metadata"), options: metadataOptions },
              { name: "sort", type: "select", label: t("books.sort"), options: sortOptions },
            ]}
          />
        </CardContent>
      </Card>

      {/* Résultats */}
      {searchQuery && totalHits !== null ? (
        <p className="text-sm text-muted-foreground mb-4">
          {t("books.resultCountFor", { count: String(totalHits), plural: totalHits !== 1 ? "s" : "", query: searchQuery })}
        </p>
      ) : !searchQuery && (
        <p className="text-sm text-muted-foreground mb-4">
          {t("books.resultCount", { count: String(total), plural: total !== 1 ? "s" : "" })}
        </p>
      )}

      {/* Séries matchantes */}
      {seriesHits.length > 0 && (
        <div className="mb-8">
          <h2 className="text-lg font-semibold text-foreground mb-3">{t("books.seriesHeading")}</h2>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
            {seriesHits.map((s) => (
              <Link
                key={`${s.library_id}-${s.name}`}
                href={`/libraries/${s.library_id}/series/${encodeURIComponent(s.name)}`}
                className="group"
              >
                <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md transition-shadow duration-200">
                  <div className="aspect-[2/3] relative bg-muted/50">
                    <Image
                      src={getBookCoverUrl(s.first_book_id)}
                      alt={t("books.coverOf", { name: s.name })}
                      fill
                      className="object-cover"
                      sizes="(max-width: 640px) 50vw, (max-width: 768px) 33vw, (max-width: 1024px) 25vw, 16vw"
                    />
                  </div>
                  <div className="p-2">
                    <h3 className="font-medium text-foreground truncate text-sm" title={s.name}>
                      {s.name === "unclassified" ? t("books.unclassified") : s.name}
                    </h3>
                    <p className="text-xs text-muted-foreground mt-0.5">
                      {t("books.bookCount", { count: String(s.book_count), plural: s.book_count !== 1 ? "s" : "" })}
                    </p>
                  </div>
                </div>
              </Link>
            ))}
          </div>
        </div>
      )}

      {/* Grille de livres */}
      {displayBooks.length > 0 ? (
        <>
          {searchQuery && <h2 className="text-lg font-semibold text-foreground mb-3">{t("books.title")}</h2>}
          <BooksGrid books={displayBooks} />

          {!searchQuery && (
            <OffsetPagination
              currentPage={page}
              totalPages={totalPages}
              pageSize={limit}
              totalItems={total}
            />
          )}
        </>
      ) : (
        <EmptyState message={searchQuery ? t("books.noResults", { query: searchQuery }) : t("books.noBooks")} />
      )}
    </>
  );
}

import { fetchBooks, fetchAllSeries, BooksPageDto, SeriesPageDto, getBookCoverUrl } from "../../../lib/api";
import { getServerTranslations } from "../../../lib/i18n/server";
import { BooksGrid } from "../../components/BookCard";
import { OffsetPagination } from "../../components/ui";
import Image from "next/image";
import Link from "next/link";

export const dynamic = "force-dynamic";

export default async function AuthorDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ name: string }>;
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const { name: encodedName } = await params;
  const authorName = decodeURIComponent(encodedName);
  const searchParamsAwaited = await searchParams;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  // Fetch books by this author (server-side filtering via API) and series
  const [booksPage, seriesPage] = await Promise.all([
    fetchBooks(undefined, undefined, page, limit, undefined, undefined, authorName).catch(
      () => ({ items: [], total: 0, page: 1, limit }) as BooksPageDto
    ),
    fetchAllSeries(undefined, undefined, undefined, 1, 200).catch(
      () => ({ items: [], total: 0, page: 1, limit: 200 }) as SeriesPageDto
    ),
  ]);

  const totalPages = Math.ceil(booksPage.total / limit);

  // Extract unique series names from this author's books
  const authorSeriesNames = new Set(
    booksPage.items
      .map((b) => b.series)
      .filter((s): s is string => s != null && s !== "")
  );

  const authorSeries = seriesPage.items.filter((s) => authorSeriesNames.has(s.name));

  return (
    <>
      {/* Breadcrumb */}
      <nav className="flex items-center gap-2 text-sm text-muted-foreground mb-6">
        <Link href="/authors" className="hover:text-foreground transition-colors">
          {t("authors.title")}
        </Link>
        <span>/</span>
        <span className="text-foreground font-medium">{authorName}</span>
      </nav>

      {/* Author Header */}
      <div className="flex items-center gap-4 mb-8">
        <div className="w-16 h-16 rounded-full bg-accent/50 flex items-center justify-center flex-shrink-0">
          <span className="text-2xl font-bold text-accent-foreground">
            {authorName.charAt(0).toUpperCase()}
          </span>
        </div>
        <div>
          <h1 className="text-3xl font-bold text-foreground">{authorName}</h1>
          <div className="flex items-center gap-4 mt-1">
            <span className="text-sm text-muted-foreground">
              {t("authors.bookCount", { count: String(booksPage.total), plural: booksPage.total !== 1 ? "s" : "" })}
            </span>
            {authorSeries.length > 0 && (
              <span className="text-sm text-muted-foreground">
                {t("authors.seriesCount", { count: String(authorSeries.length), plural: authorSeries.length !== 1 ? "s" : "" })}
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Series Section */}
      {authorSeries.length > 0 && (
        <section className="mb-8">
          <h2 className="text-xl font-semibold text-foreground mb-4">
            {t("authors.seriesBy", { name: authorName })}
          </h2>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
            {authorSeries.map((s) => (
              <Link
                key={`${s.library_id}-${s.name}`}
                href={`/libraries/${s.library_id}/series/${encodeURIComponent(s.name)}`}
                className="group"
              >
                <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md hover:-translate-y-1 transition-all duration-200">
                  <div className="aspect-[2/3] relative bg-muted/50">
                    <Image
                      src={getBookCoverUrl(s.first_book_id)}
                      alt={s.name}
                      fill
                      className="object-cover"
                      unoptimized
                    />
                  </div>
                  <div className="p-3">
                    <h3 className="font-medium text-foreground truncate text-sm" title={s.name}>
                      {s.name}
                    </h3>
                    <p className="text-xs text-muted-foreground mt-1">
                      {t("authors.bookCount", { count: String(s.book_count), plural: s.book_count !== 1 ? "s" : "" })}
                    </p>
                  </div>
                </div>
              </Link>
            ))}
          </div>
        </section>
      )}

      {/* Books Section */}
      {booksPage.items.length > 0 && (
        <section>
          <h2 className="text-xl font-semibold text-foreground mb-4">
            {t("authors.booksBy", { name: authorName })}
          </h2>
          <BooksGrid books={booksPage.items} />
          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={booksPage.total}
          />
        </section>
      )}

      {/* Empty State */}
      {booksPage.items.length === 0 && authorSeries.length === 0 && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <p className="text-muted-foreground text-lg">
            {t("authors.noResults")}
          </p>
        </div>
      )}
    </>
  );
}

import { fetchAllSeries, fetchLibraries, LibraryDto, SeriesDto, SeriesPageDto, getBookCoverUrl } from "../../lib/api";
import { MarkSeriesReadButton } from "../components/MarkSeriesReadButton";
import { LiveSearchForm } from "../components/LiveSearchForm";
import { Card, CardContent, OffsetPagination } from "../components/ui";
import Image from "next/image";
import Link from "next/link";

export const dynamic = "force-dynamic";

export default async function SeriesPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const searchParamsAwaited = await searchParams;
  const libraryId = typeof searchParamsAwaited.library === "string" ? searchParamsAwaited.library : undefined;
  const searchQuery = typeof searchParamsAwaited.q === "string" ? searchParamsAwaited.q : "";
  const readingStatus = typeof searchParamsAwaited.status === "string" ? searchParamsAwaited.status : undefined;
  const sort = typeof searchParamsAwaited.sort === "string" ? searchParamsAwaited.sort : undefined;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const [libraries, seriesPage] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[]),
    fetchAllSeries(libraryId, searchQuery || undefined, readingStatus, page, limit, sort).catch(
      () => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto
    ),
  ]);

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);
  const sortOptions = [
    { value: "", label: "Title" },
    { value: "latest", label: "Latest added" },
  ];

  const hasFilters = searchQuery || libraryId || readingStatus || sort;

  const libraryOptions = [
    { value: "", label: "All libraries" },
    ...libraries.map((lib) => ({ value: lib.id, label: lib.name })),
  ];

  const statusOptions = [
    { value: "", label: "All" },
    { value: "unread", label: "Unread" },
    { value: "reading", label: "In progress" },
    { value: "read", label: "Read" },
  ];

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          Series
        </h1>
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <LiveSearchForm
            basePath="/series"
            fields={[
              { name: "q", type: "text", label: "Search", placeholder: "Search by series name...", className: "flex-1 w-full" },
              { name: "library", type: "select", label: "Library", options: libraryOptions, className: "w-full sm:w-48" },
              { name: "status", type: "select", label: "Status", options: statusOptions, className: "w-full sm:w-40" },
              { name: "sort", type: "select", label: "Sort", options: sortOptions, className: "w-full sm:w-40" },
            ]}
          />
        </CardContent>
      </Card>

      {/* Results count */}
      <p className="text-sm text-muted-foreground mb-4">
        {seriesPage.total} series
        {searchQuery && <> matching &quot;{searchQuery}&quot;</>}
      </p>

      {/* Series Grid */}
      {series.length > 0 ? (
        <>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
            {series.map((s) => (
              <Link
                key={s.name}
                href={`/libraries/${s.library_id}/series/${encodeURIComponent(s.name)}`}
                className="group"
              >
                <div
                  className={`bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md hover:-translate-y-1 transition-all duration-200 ${
                    s.books_read_count >= s.book_count ? "opacity-50" : ""
                  }`}
                >
                  <div className="aspect-[2/3] relative bg-muted/50">
                    <Image
                      src={getBookCoverUrl(s.first_book_id)}
                      alt={`Cover of ${s.name}`}
                      fill
                      className="object-cover"
                      unoptimized
                    />
                  </div>
                  <div className="p-3">
                    <h3 className="font-medium text-foreground truncate text-sm" title={s.name}>
                      {s.name === "unclassified" ? "Unclassified" : s.name}
                    </h3>
                    <div className="flex items-center justify-between mt-1">
                      <p className="text-xs text-muted-foreground">
                        {s.books_read_count}/{s.book_count} lu{s.book_count !== 1 ? "s" : ""}
                      </p>
                      <MarkSeriesReadButton
                        seriesName={s.name}
                        bookCount={s.book_count}
                        booksReadCount={s.books_read_count}
                      />
                    </div>
                  </div>
                </div>
              </Link>
            ))}
          </div>

          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={seriesPage.total}
          />
        </>
      ) : (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-16 h-16 mb-4 text-muted-foreground/30">
            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
          </div>
          <p className="text-muted-foreground text-lg">
            {hasFilters ? "No series found matching your filters" : "No series available"}
          </p>
        </div>
      )}
    </>
  );
}

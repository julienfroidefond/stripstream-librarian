import { fetchLibraries, fetchSeries, getBookCoverUrl, LibraryDto, SeriesDto, SeriesPageDto } from "../../../../lib/api";
import { OffsetPagination } from "../../../components/ui";
import { MarkSeriesReadButton } from "../../../components/MarkSeriesReadButton";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { LibrarySubPageHeader } from "../../../components/LibrarySubPageHeader";

export const dynamic = "force-dynamic";

export default async function LibrarySeriesPage({
  params,
  searchParams
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { id } = await params;
  const searchParamsAwaited = await searchParams;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const [library, seriesPage] = await Promise.all([
    fetchLibraries().then(libs => libs.find(l => l.id === id)),
    fetchSeries(id, page, limit).catch(() => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto)
  ]);

  if (!library) {
    notFound();
  }

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);

  return (
    <div className="space-y-6">
      <LibrarySubPageHeader
        library={library}
        title="Series"
        icon={
          <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        }
        iconColor="text-primary"
      />

      {series.length > 0 ? (
        <>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-6">
            {series.map((s) => (
              <Link
                key={s.name}
                href={`/libraries/${id}/series/${encodeURIComponent(s.name)}`}
                className="group"
              >
                <div className={`bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md transition-shadow duration-200 ${s.books_read_count >= s.book_count ? "opacity-50" : ""}`}>
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
                        {s.books_read_count}/{s.book_count} lu{s.book_count !== 1 ? 's' : ''}
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
        <div className="text-center py-12 text-muted-foreground">
          <p>No series found in this library</p>
        </div>
      )}
    </div>
  );
}

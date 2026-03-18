import { fetchLibraries, fetchSeries, fetchSeriesStatuses, getBookCoverUrl, LibraryDto, SeriesDto, SeriesPageDto } from "../../../../lib/api";
import { OffsetPagination } from "../../../components/ui";
import { MarkSeriesReadButton } from "../../../components/MarkSeriesReadButton";
import { SeriesFilters } from "../../../components/SeriesFilters";
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
  const seriesStatus = typeof searchParamsAwaited.series_status === "string" ? searchParamsAwaited.series_status : undefined;
  const hasMissing = searchParamsAwaited.has_missing === "true";

  const [library, seriesPage, dbStatuses] = await Promise.all([
    fetchLibraries().then(libs => libs.find(l => l.id === id)),
    fetchSeries(id, page, limit, seriesStatus, hasMissing).catch(() => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto),
    fetchSeriesStatuses().catch(() => [] as string[]),
  ]);

  if (!library) {
    notFound();
  }

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);

  const KNOWN_STATUSES: Record<string, string> = {
    ongoing: "En cours",
    ended: "Terminée",
    hiatus: "Hiatus",
    cancelled: "Annulée",
    upcoming: "À paraître",
  };
  const seriesStatusOptions = [
    { value: "", label: "Tous les statuts" },
    ...dbStatuses.map((s) => ({ value: s, label: KNOWN_STATUSES[s] || s })),
  ];

  return (
    <div className="space-y-6">
      <LibrarySubPageHeader
        library={library}
        title="Séries"
        icon={
          <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        }
        iconColor="text-primary"
      />

      <SeriesFilters
        basePath={`/libraries/${id}/series`}
        currentSeriesStatus={seriesStatus}
        currentHasMissing={hasMissing}
        seriesStatusOptions={seriesStatusOptions}
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
                      alt={`Couverture de ${s.name}`}
                      fill
                      className="object-cover"
                      unoptimized
                    />
                  </div>
                  <div className="p-3">
                    <h3 className="font-medium text-foreground truncate text-sm" title={s.name}>
                      {s.name === "unclassified" ? "Non classé" : s.name}
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
                    <div className="flex items-center gap-1 mt-1.5 flex-wrap">
                      {s.series_status && (
                        <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                          s.series_status === "ongoing" ? "bg-blue-500/15 text-blue-600" :
                          s.series_status === "ended" ? "bg-green-500/15 text-green-600" :
                          s.series_status === "hiatus" ? "bg-amber-500/15 text-amber-600" :
                          s.series_status === "cancelled" ? "bg-red-500/15 text-red-600" :
                          "bg-muted text-muted-foreground"
                        }`}>
                          {s.series_status === "ongoing" ? "En cours" :
                           s.series_status === "ended" ? "Terminée" :
                           s.series_status === "hiatus" ? "Hiatus" :
                           s.series_status === "cancelled" ? "Annulée" :
                           s.series_status === "upcoming" ? "À paraître" :
                           s.series_status}
                        </span>
                      )}
                      {s.missing_count != null && s.missing_count > 0 && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-yellow-500/15 text-yellow-600">
                          {s.missing_count} manquant{s.missing_count > 1 ? "s" : ""}
                        </span>
                      )}
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
          <p>Aucune série trouvée dans cette bibliothèque</p>
        </div>
      )}
    </div>
  );
}

import { fetchAllSeries, fetchLibraries, fetchSeriesStatuses, LibraryDto, SeriesDto, SeriesPageDto, getBookCoverUrl } from "../../lib/api";
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
  const seriesStatus = typeof searchParamsAwaited.series_status === "string" ? searchParamsAwaited.series_status : undefined;
  const hasMissing = searchParamsAwaited.has_missing === "true";
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const [libraries, seriesPage, dbStatuses] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[]),
    fetchAllSeries(libraryId, searchQuery || undefined, readingStatus, page, limit, sort, seriesStatus, hasMissing).catch(
      () => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto
    ),
    fetchSeriesStatuses().catch(() => [] as string[]),
  ]);

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);
  const sortOptions = [
    { value: "", label: "Titre" },
    { value: "latest", label: "Ajout récent" },
  ];

  const hasFilters = searchQuery || libraryId || readingStatus || sort || seriesStatus || hasMissing;

  const libraryOptions = [
    { value: "", label: "Toutes les bibliothèques" },
    ...libraries.map((lib) => ({ value: lib.id, label: lib.name })),
  ];

  const statusOptions = [
    { value: "", label: "Tous" },
    { value: "unread", label: "Non lu" },
    { value: "reading", label: "En cours" },
    { value: "read", label: "Lu" },
  ];

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

  const missingOptions = [
    { value: "", label: "Tous" },
    { value: "true", label: "Livres manquants" },
  ];

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          Séries
        </h1>
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <LiveSearchForm
            basePath="/series"
            fields={[
              { name: "q", type: "text", label: "Rechercher", placeholder: "Rechercher par nom de série...", className: "flex-1 w-full" },
              { name: "library", type: "select", label: "Bibliothèque", options: libraryOptions, className: "w-full sm:w-48" },
              { name: "status", type: "select", label: "Lecture", options: statusOptions, className: "w-full sm:w-36" },
              { name: "series_status", type: "select", label: "Statut", options: seriesStatusOptions, className: "w-full sm:w-36" },
              { name: "has_missing", type: "select", label: "Manquant", options: missingOptions, className: "w-full sm:w-36" },
              { name: "sort", type: "select", label: "Tri", options: sortOptions, className: "w-full sm:w-36" },
            ]}
          />
        </CardContent>
      </Card>

      {/* Results count */}
      <p className="text-sm text-muted-foreground mb-4">
        {seriesPage.total} séries
        {searchQuery && <> correspondant à &quot;{searchQuery}&quot;</>}
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
                        {s.books_read_count}/{s.book_count} lu{s.book_count !== 1 ? "s" : ""}
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
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-16 h-16 mb-4 text-muted-foreground/30">
            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
          </div>
          <p className="text-muted-foreground text-lg">
            {hasFilters ? "Aucune série trouvée correspondant à vos filtres" : "Aucune série disponible"}
          </p>
        </div>
      )}
    </>
  );
}

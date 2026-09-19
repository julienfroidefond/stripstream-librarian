import { fetchLibraries, fetchSeriesById, fetchSeriesMetadata, getMissingBooks, apiFetch, fetchDownloadsEnabled, SeriesMetadataDto, MissingBooksDto, TelegramMonitorStatus } from "@/lib/api";
import { MissingBookActionsToolbar } from "@/app/components/MissingBookActionsToolbar";
import { SafeHtml } from "@/app/components/SafeHtml";
import { getServerTranslations } from "@/lib/i18n/server";
import Link from "next/link";
import { notFound } from "next/navigation";

export const dynamic = "force-dynamic";

export default async function MissingBookDetailPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const sp = await searchParams;
  const seriesId = typeof sp.seriesId === "string" ? sp.seriesId : null;
  const linkId = typeof sp.linkId === "string" ? sp.linkId : null;
  const volume = typeof sp.volume === "string" ? parseInt(sp.volume, 10) : null;
  const titleParam = typeof sp.title === "string" ? sp.title : null;

  if (!seriesId || !linkId) {
    notFound();
  }

  const [seriesDto, missingData, libraries, seriesMeta, prowlarrConfigured, qbConfigured, telegramStatus] = await Promise.all([
    fetchSeriesById(seriesId).catch(() => null),
    getMissingBooks(linkId).catch(() => null as MissingBooksDto | null),
    fetchLibraries().catch(() => [] as { id: string; name: string }[]),
    fetchSeriesMetadata(seriesId).catch(() => null as SeriesMetadataDto | null),
    fetchDownloadsEnabled(),
    apiFetch<{ url?: string; username?: string }>("/settings/qbittorrent", { next: { revalidate: 3600 } })
      .then(d => !!(d?.url?.trim() && d?.username?.trim()))
      .catch(() => false),
    apiFetch<TelegramMonitorStatus>("/telegram-monitor/status", { next: { revalidate: 60 } }).catch(() => null),
  ]);

  if (!seriesDto) {
    notFound();
  }

  const telegramEnabled = !!(telegramStatus?.configured && telegramStatus?.authorized);
  const library = libraries.find(l => l.id === seriesDto.library_id);
  const seriesName = seriesMeta?.series_name ?? seriesDto.name;

  const missingList = missingData?.missing_books ?? [];
  const missing =
    missingList.find((m) => (volume != null ? m.volume_number === volume : titleParam != null && m.title === titleParam)) ??
    missingList.find((m) => volume != null && m.volume_number === volume) ??
    null;

  const title = missing?.title ?? titleParam ?? (volume != null ? `${seriesName} T${volume}` : seriesName);
  const coverUrl = missing?.cover_url ?? null;
  const missingBook = {
    title: missing?.title ?? title,
    volume_number: missing?.volume_number ?? volume,
    external_book_id: missing?.external_book_id ?? null,
  };

  const { t } = await getServerTranslations();

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
              href={`/libraries/${seriesDto.library_id}/series`}
              className="text-muted-foreground hover:text-primary transition-colors"
            >
              {library.name}
            </Link>
            <span className="text-muted-foreground">/</span>
          </>
        )}
        <Link
          href={`/series/${seriesId}`}
          className="text-muted-foreground hover:text-primary transition-colors"
        >
          {seriesName}
        </Link>
        <span className="text-muted-foreground">/</span>
        <span className="text-foreground font-medium truncate">{title}</span>
      </div>

      {/* Hero */}
      <div className="flex flex-col sm:flex-row gap-6">
        <div className="flex-shrink-0">
          <div className="w-40 aspect-[2/3] relative rounded-xl overflow-hidden shadow-card border border-dashed border-border">
            {coverUrl ? (
              /* eslint-disable-next-line @next/next/no-img-element */
              <img src={coverUrl} alt={title} className="w-full h-full object-cover grayscale" />
            ) : (
              <div className="flex items-center justify-center h-full bg-muted">
                <svg className="w-10 h-10 text-muted-foreground/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                </svg>
              </div>
            )}
          </div>
        </div>

        <div className="flex-1 space-y-4">
          <h1 className="text-3xl font-bold text-foreground">{title}</h1>

          <div className="flex flex-wrap items-center gap-3">
            <Link
              href={`/series/${seriesId}`}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs border border-primary/30 font-medium"
            >
              {seriesName}
              {missingBook.volume_number != null && <span className="font-semibold">Vol. {missingBook.volume_number}</span>}
            </Link>
            <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-muted/60 text-muted-foreground border border-border">
              {t("status.missing")}
            </span>
            {seriesMeta && seriesMeta.authors.length > 0 && (
              <p className="text-base text-muted-foreground">{seriesMeta.authors.join(", ")}</p>
            )}
          </div>

          {seriesMeta?.description && (
            <SafeHtml html={seriesMeta.description} className="text-sm text-muted-foreground leading-relaxed" />
          )}

          <MissingBookActionsToolbar
            seriesName={seriesName}
            libraryId={seriesDto.library_id}
            volumeNumber={missingBook.volume_number}
            missingBook={missingBook}
            prowlarrConfigured={prowlarrConfigured}
            qbConfigured={qbConfigured}
            telegramEnabled={telegramEnabled}
          />
        </div>
      </div>
    </div>
  );
}

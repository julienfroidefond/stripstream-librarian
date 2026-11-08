import {
  FilterBarSkeleton,
  PageHeaderSkeleton,
  PageTabsSkeleton,
  TableCardSkeleton,
} from "@/app/components/PageSkeleton";
import { Skeleton } from "@/app/components/ui";
import { getServerTranslations } from "@/lib/i18n/server";

/** One placeholder width per column of the metadata tables. */
const METADATA_COLUMNS = [
  "h-12 w-9", // cover
  "h-4 w-40", // name / title
  "h-4 w-20", // provider / series
  "h-4 w-16", // status / volume
  "h-4 w-10", // year / authors
  "h-4 w-32", // genres / language
  "h-4 w-8", // volumes / pages
  "h-4 w-8", // missing / format
  "h-4 w-12", // rating / summary
];

/**
 * Instant loading boundary for the metadata-gaps page.
 *
 * Navigating between gap chips / tabs re-renders the whole server segment.
 * Without a `loading.tsx`, the UI stays frozen on the previous screen until the
 * new RSC payload arrives. This skeleton gives immediate feedback (and enables
 * the prefetch/streaming boundary) so filter clicks never feel stuck.
 */
export default async function MetadataLoading() {
  const { t } = await getServerTranslations();

  return (
    <div aria-busy="true">
      <span className="sr-only">{t("common.loading")}</span>

      <PageHeaderSkeleton titleWidth="w-64" subtitle />
      <PageTabsSkeleton count={2} widths={["w-20", "w-16"]} compact />
      <FilterBarSkeleton filters={1} />

      {/* Gap chips: "all" + one per gap of the current tab. */}
      <div data-testid="loading-gap-chips" className="mb-4 flex flex-wrap gap-2">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-8 w-32 rounded-full" />
        ))}
      </div>

      <div data-testid="loading-result-count">
        <Skeleton className="mb-4 h-4 w-40" />
      </div>

      <TableCardSkeleton rows={8} columns={METADATA_COLUMNS} />
    </div>
  );
}

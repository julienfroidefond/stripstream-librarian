import { getServerTranslations } from "@/lib/i18n/server";
import { Card, CardContent, Skeleton } from "@/app/components/ui";

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

      <div className="mb-6">
        <Skeleton className="h-9 w-64 max-w-full" />
        <Skeleton className="mt-3 h-4 w-96 max-w-full" />
      </div>

      <div className="mb-6 flex gap-6 border-b border-border/60 pb-3">
        <Skeleton className="h-5 w-24" />
        <Skeleton className="h-5 w-24" />
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <div className="flex flex-col gap-4 sm:flex-row">
            <Skeleton className="h-10 flex-1" />
            <Skeleton className="h-10 w-full sm:w-56" />
          </div>
        </CardContent>
      </Card>

      <div className="mb-4 flex flex-wrap gap-2">
        {Array.from({ length: 7 }).map((_, i) => (
          <Skeleton key={i} className="h-8 w-32 rounded-full" />
        ))}
      </div>

      <Card>
        <CardContent className="divide-y divide-border/60 p-0">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="px-4 py-3">
              <Skeleton className="h-4 w-1/3" />
              <Skeleton className="mt-2 h-3 w-1/2" />
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}

import { getServerTranslations } from "@/lib/i18n/server";
import { Card, Skeleton } from "@/app/components/ui";

const USER_TABS = 3;
const CURRENTLY_READING_CARDS = 5;
const SERIES_ROWS = 4;
const RECENT_ROWS = 4;
const STAT_TILES = 3;

function SectionTitleSkeleton() {
  return <Skeleton className="mb-3 h-4 w-32" />;
}

export default async function ProgressLoading() {
  const { t } = await getServerTranslations();

  return (
    <div aria-busy="true">
      <span className="sr-only">{t("common.loading")}</span>

      <div data-testid="loading-header" className="mb-6 flex items-center gap-3">
        <Skeleton className="h-8 w-8 rounded-lg" />
        <Skeleton className="h-9 w-40" />
      </div>

      <div className="space-y-4">
        <div data-testid="loading-user-tabs" className="flex gap-2 overflow-hidden">
          {Array.from({ length: USER_TABS }).map((_, i) => (
            <Skeleton key={i} className="h-10 w-36 shrink-0 rounded-full" />
          ))}
        </div>

        <Card hover={false} className="p-4 sm:p-6">
          <div data-testid="loading-stats" className="grid grid-cols-3 gap-2 sm:gap-3">
            {Array.from({ length: STAT_TILES }).map((_, i) => (
              <Skeleton key={i} className="h-16 rounded-lg sm:h-[4.5rem]" />
            ))}
          </div>

          <div data-testid="loading-currently-reading" className="mt-6">
            <SectionTitleSkeleton />
            <div className="flex gap-3 overflow-hidden pb-2 sm:gap-4">
              {Array.from({ length: CURRENTLY_READING_CARDS }).map((_, i) => (
                <div key={i} className="w-28 shrink-0 sm:w-32">
                  <Skeleton className="aspect-[2/3] w-full rounded-lg" />
                  <div className="mt-2 flex items-center gap-2">
                    <Skeleton className="h-1 flex-1 rounded-full" />
                    <Skeleton className="h-2 w-6" />
                  </div>
                  <Skeleton className="mt-2 h-3 w-5/6" />
                  <Skeleton className="mt-1.5 h-2.5 w-2/3" />
                </div>
              ))}
            </div>
          </div>

          <div className="mt-6 grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
            <div data-testid="loading-series">
              <SectionTitleSkeleton />
              <div className="divide-y divide-border/50">
                {Array.from({ length: SERIES_ROWS }).map((_, i) => (
                  <div key={i} className="flex items-center gap-3 py-2.5">
                    <Skeleton className="h-12 w-8 shrink-0 rounded" />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-baseline justify-between gap-3">
                        <Skeleton className="h-3.5 w-1/2" />
                        <Skeleton className="h-3 w-8" />
                      </div>
                      <Skeleton className="mt-2 h-1.5 w-full rounded-full" />
                      <Skeleton className="mt-2 h-2.5 w-1/3" />
                    </div>
                  </div>
                ))}
              </div>
            </div>

            <div data-testid="loading-recent">
              <SectionTitleSkeleton />
              <div className="space-y-1">
                {Array.from({ length: RECENT_ROWS }).map((_, i) => (
                  <div key={i} className="flex items-center gap-3 py-1.5">
                    <Skeleton className="h-[42px] w-7 shrink-0 rounded" />
                    <div className="min-w-0 flex-1">
                      <Skeleton className="h-3.5 w-4/5" />
                      <Skeleton className="mt-1.5 h-2.5 w-1/2" />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </Card>
      </div>
    </div>
  );
}

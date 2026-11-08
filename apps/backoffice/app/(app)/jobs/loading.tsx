import { PageHeaderSkeleton, TableCardSkeleton } from "@/app/components/PageSkeleton";
import { Skeleton } from "@/app/components/ui";
import { getServerTranslations } from "@/lib/i18n/server";

/** One placeholder width per column of the jobs table. */
const JOB_COLUMNS = [
  "h-4 w-16", // id
  "h-4 w-24", // library
  "h-4 w-20", // type
  "h-4 w-16", // status
  "h-4 w-40", // stats
  "h-4 w-16", // duration
  "h-4 w-20", // created
  "h-4 w-16", // actions
];

export default async function JobsLoading() {
  const { t } = await getServerTranslations();

  return (
    <div aria-busy="true">
      <span className="sr-only">{t("common.loading")}</span>

      <PageHeaderSkeleton titleWidth="w-40" />

      {/* JobActions: library selector on the left, one-click actions on the right. */}
      <div data-testid="loading-job-actions" className="mb-6">
        <div className="flex flex-wrap items-center gap-2 rounded-xl border border-border/60 bg-card/50 p-2">
          <Skeleton className="h-9 w-full rounded-lg sm:w-64" />
          <div className="ml-auto flex flex-wrap items-center gap-1.5">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-9 w-9 rounded-lg sm:w-24" />
            ))}
            <Skeleton className="h-9 w-24 rounded-lg" />
          </div>
        </div>
      </div>

      <TableCardSkeleton rows={10} columns={JOB_COLUMNS} filterBar />
    </div>
  );
}

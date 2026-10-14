import { Skeleton } from "@/app/components/ui";
import { getServerTranslations } from "@/lib/i18n/server";

interface PageSkeletonProps {
  /** Shape of the main content area below the toolbar. */
  variant?: "table" | "grid";
  /** Number of placeholder rows (table) or cards (grid). */
  rows?: number;
}

/**
 * Generic route-level loading placeholder (title + toolbar + content), used by
 * the heavy routes' `loading.tsx` so client-side navigation gives immediate
 * feedback instead of freezing on the previous screen.
 */
export async function PageSkeleton({ variant = "table", rows = 8 }: PageSkeletonProps) {
  const { t } = await getServerTranslations();

  return (
    <div aria-busy="true">
      <span className="sr-only">{t("common.loading")}</span>

      <Skeleton className="mb-6 h-9 w-56" />

      <div className="mb-6 flex flex-wrap items-center gap-3">
        <Skeleton className="h-10 w-full max-w-md" />
        <Skeleton className="h-10 w-40" />
        <Skeleton className="ml-auto h-10 w-28" />
      </div>

      {variant === "grid" ? (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {Array.from({ length: rows }).map((_, i) => (
            <Skeleton key={i} className="aspect-[2/3] rounded-lg" />
          ))}
        </div>
      ) : (
        <div className="overflow-hidden rounded-lg border border-border/60">
          {Array.from({ length: rows }).map((_, i) => (
            <div
              key={i}
              className="flex items-center gap-4 border-b border-border/60 px-4 py-3 last:border-0"
            >
              <Skeleton className="h-4 w-16" />
              <Skeleton className="h-4 flex-1" />
              <Skeleton className="hidden h-4 w-24 sm:block" />
              <Skeleton className="h-4 w-16" />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

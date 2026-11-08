import { Skeleton } from "@/app/components/ui";
import { getServerTranslations } from "@/lib/i18n/server";

/**
 * Composable loading placeholders shared by the route-level `loading.tsx`
 * files. Each primitive mirrors the markup of the real component it stands in
 * for (page header, `LiveSearchForm`, section tabs, results row, cover grid,
 * data table) so the swap from skeleton to content does not shift the layout.
 */

/** Page title row: `text-3xl` heading with its icon, optional subtitle and actions. */
export function PageHeaderSkeleton({
  titleWidth = "w-56",
  subtitle = false,
  actions = 0,
}: {
  titleWidth?: string;
  subtitle?: boolean;
  actions?: number;
}) {
  return (
    <div data-testid="page-header-skeleton" className="mb-6 flex items-center justify-between gap-3">
      <div className="min-w-0">
        <div className="flex items-center gap-3">
          <Skeleton className="h-8 w-8 shrink-0 rounded-lg" />
          <Skeleton className={`h-9 ${titleWidth}`} />
        </div>
        {subtitle && <Skeleton className="mt-3 h-4 w-96 max-w-full" />}
      </div>
      {actions > 0 && (
        <div className="flex shrink-0 items-center gap-2">
          {Array.from({ length: actions }).map((_, i) => (
            <Skeleton key={i} className="h-9 w-28 rounded-md" />
          ))}
        </div>
      )}
    </div>
  );
}

/** Section tabs (`nav` with an active underline), as used by series and metadata. */
export function PageTabsSkeleton({
  count = 3,
  widths = [],
  compact = false,
}: {
  count?: number;
  /** Optional per-tab label width (`w-24`, …). */
  widths?: string[];
  /** Metadata-style tabs: tighter padding and gap. */
  compact?: boolean;
}) {
  return (
    <div
      data-testid="page-tabs-skeleton"
      className={`mb-6 flex border-b ${compact ? "gap-1 border-border/60" : "gap-0 border-border"}`}
    >
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className={`-mb-px flex items-center gap-2 border-b-2 border-transparent ${
            compact ? "px-4 py-2" : "px-4 py-2.5"
          }`}
        >
          <Skeleton className="h-4 w-4 rounded" />
          <Skeleton className={`h-4 ${widths[i] ?? "w-24"}`} />
        </div>
      ))}
    </div>
  );
}

const FILTER_PILL_WIDTHS = ["w-24", "w-28", "w-32", "w-24", "w-28", "w-32", "w-24", "w-28"];

/**
 * `LiveSearchForm` placeholder: the card, the full-width search input and the
 * row of filter pills (collapsed behind the mobile toggle below `sm`).
 */
export function FilterBarSkeleton({ filters = 0 }: { filters?: number }) {
  return (
    <div
      data-testid="filter-bar-skeleton"
      className="mb-6 space-y-3 rounded-xl border border-border/60 bg-card p-3 shadow-soft"
    >
      <Skeleton className="h-11 w-full rounded-xl" />
      {filters > 0 && (
        <>
          <div className="hidden flex-wrap items-center gap-2 sm:flex">
            {Array.from({ length: filters }).map((_, i) => (
              <Skeleton
                key={i}
                className={`h-9 rounded-full ${FILTER_PILL_WIDTHS[i % FILTER_PILL_WIDTHS.length]}`}
              />
            ))}
          </div>
          <Skeleton className="h-9 w-24 rounded-full sm:hidden" />
        </>
      )}
    </div>
  );
}

/** Results count + grid/list toggle row above the catalog results. */
export function ResultsRowSkeleton() {
  return (
    <div data-testid="results-row-skeleton" className="mb-4 flex items-center justify-between gap-3">
      <Skeleton className="h-4 w-40" />
      <Skeleton className="h-9 w-24 rounded-full" />
    </div>
  );
}

/** Catalog cover grid: same responsive columns and card chrome as `CoverCard`. */
export function CoverGridSkeleton({ rows = 12 }: { rows?: number }) {
  return (
    <div
      data-testid="cover-grid-skeleton"
      className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6"
    >
      {Array.from({ length: rows }).map((_, i) => (
        <Skeleton key={i} className="aspect-[2/3] rounded-xl shadow-sm ring-1 ring-border/50" />
      ))}
    </div>
  );
}

/**
 * Card-wrapped data table placeholder (jobs, metadata). `columns` holds one
 * cell width class per column; the header row is derived from its length.
 */
export function TableCardSkeleton({
  rows = 8,
  columns,
  filterBar = false,
}: {
  rows?: number;
  columns: string[];
  /** Show the toolbar above the table (jobs filter selects + counter). */
  filterBar?: boolean;
}) {
  return (
    <div
      data-testid="table-card-skeleton"
      className="overflow-hidden rounded-xl border border-border/60 bg-card shadow-sm"
    >
      {filterBar && (
        <div className="flex flex-wrap items-center gap-3 border-b border-border/60 bg-muted/30 px-4 py-3">
          <Skeleton className="h-7 w-28 rounded-md" />
          <Skeleton className="h-7 w-28 rounded-md" />
          <Skeleton className="h-7 w-28 rounded-md" />
          <Skeleton className="ml-auto h-4 w-16" />
        </div>
      )}
      {/* The real tables scroll horizontally inside their card below `min-w`. */}
      <div className="overflow-x-auto">
        <div className="divide-y divide-border/60">
          <div className="flex items-center gap-4 bg-muted/50 px-4 py-3">
            {columns.map((_, i) => (
              <Skeleton key={i} className="h-3 w-16 shrink-0" />
            ))}
          </div>
          {Array.from({ length: rows }).map((_, row) => (
            <div key={row} className="flex items-center gap-4 px-4 py-3">
              {columns.map((width, i) => (
                <Skeleton key={i} className={`${width} shrink-0`} />
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

const DEFAULT_TABLE_COLUMNS = ["h-4 w-16", "h-4 w-48", "h-4 w-24", "h-4 w-16"];

interface PageSkeletonProps {
  /** Shape of the main content area below the header. */
  variant?: "table" | "grid";
  /** Number of placeholder rows (table) or cards (grid). */
  rows?: number;
  /** Filter pills in the `LiveSearchForm` placeholder (0 hides the bar). */
  filters?: number;
  /** Tabs in the section nav placeholder (0 hides the tabs). */
  tabs?: number;
  /** Per-tab label widths, forwarded to `PageTabsSkeleton`. */
  tabWidths?: string[];
  /** Header action buttons. */
  actions?: number;
  /** Show the results-count + view-toggle row. */
  results?: boolean;
  /** Show a subtitle under the page title. */
  subtitle?: boolean;
  /** Column widths for the table variant. */
  columns?: string[];
}

/**
 * Generic route-level loading placeholder (title + toolbar + content), used by
 * the heavy routes' `loading.tsx` so client-side navigation gives immediate
 * feedback instead of freezing on the previous screen.
 */
export async function PageSkeleton({
  variant = "table",
  rows = 8,
  filters = 0,
  tabs = 0,
  tabWidths,
  actions = 0,
  results = false,
  subtitle = false,
  columns = DEFAULT_TABLE_COLUMNS,
}: PageSkeletonProps) {
  const { t } = await getServerTranslations();

  return (
    <div data-testid="page-skeleton" aria-busy="true">
      <span className="sr-only">{t("common.loading")}</span>

      <PageHeaderSkeleton subtitle={subtitle} actions={actions} />

      {tabs > 0 && <PageTabsSkeleton count={tabs} widths={tabWidths} />}

      {filters > 0 && <FilterBarSkeleton filters={filters} />}

      {results && <ResultsRowSkeleton />}

      {variant === "grid" ? (
        <CoverGridSkeleton rows={rows} />
      ) : (
        <TableCardSkeleton rows={rows} columns={columns} />
      )}
    </div>
  );
}

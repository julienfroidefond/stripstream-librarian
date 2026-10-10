"use client";

import { useRouter } from "next/navigation";
import { useTransition } from "react";
import { Button } from "./Button";
import { IconButton } from "./Button";
import { useTranslation } from "@/lib/i18n/context";

/** Read current URL search params from the browser location (safe in event handlers). */
function getCurrentParams(): URLSearchParams {
  if (typeof window === "undefined") return new URLSearchParams();
  return new URLSearchParams(window.location.search);
}

/** Navigate by updating the URL and forcing a server re-render.
 *  Works around Next.js 16 production bug where router.push/replace silently fails. */
function useNavigate() {
  const router = useRouter();
  const [, startTransition] = useTransition();
  return (url: string) => {
    window.history.pushState(null, "", url);
    startTransition(() => { router.refresh(); });
  };
}

interface OffsetPaginationProps {
  currentPage: number;
  totalPages: number;
  pageSize: number;
  totalItems: number;
  pageSizeOptions?: number[];
}

export function OffsetPagination({
  currentPage,
  totalPages,
  pageSize,
  totalItems,
  pageSizeOptions = [24, 48, 96],
}: OffsetPaginationProps) {
  const navigate = useNavigate();
  const { t } = useTranslation();

  const goToPage = (page: number) => {
    const params = getCurrentParams();
    params.set("page", page.toString());
    navigate(`?${params.toString()}`);
  };

  const changePageSize = (size: number) => {
    const params = getCurrentParams();
    params.set("limit", size.toString());
    params.set("page", "1");
    navigate(`?${params.toString()}`);
  };

  const startItem = (currentPage - 1) * pageSize + 1;
  const endItem = Math.min(currentPage * pageSize, totalItems);

  const getPageNumbers = () => {
    const pages: (number | string)[] = [];
    const maxVisiblePages = 5;

    if (totalPages <= maxVisiblePages) {
      for (let i = 1; i <= totalPages; i++) {
        pages.push(i);
      }
    } else {
      if (currentPage <= 3) {
        for (let i = 1; i <= 4; i++) {
          pages.push(i);
        }
        pages.push("...");
        pages.push(totalPages);
      } else if (currentPage >= totalPages - 2) {
        pages.push(1);
        pages.push("...");
        for (let i = totalPages - 3; i <= totalPages; i++) {
          pages.push(i);
        }
      } else {
        pages.push(1);
        pages.push("...");
        for (let i = currentPage - 1; i <= currentPage + 1; i++) {
          pages.push(i);
        }
        pages.push("...");
        pages.push(totalPages);
      }
    }
    return pages;
  };

  return (
    <div className="flex flex-col sm:flex-row items-center justify-between gap-6 mt-8 pt-8 border-t border-border/60">
      {/* Page size selector */}
      <div className="flex items-center gap-3">
        <span className="text-sm text-muted-foreground">{t("pagination.show")}</span>
        <select
          value={pageSize.toString()}
          onChange={(e) => changePageSize(Number(e.target.value))}
          className="w-20 px-3 py-2 text-sm rounded-md border border-input bg-background text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 transition-colors"
        >
          {pageSizeOptions.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
        <span className="text-sm text-muted-foreground">{t("common.perPage")}</span>
      </div>

      {/* Page info */}
      <div className="text-sm text-muted-foreground">
        {t("pagination.range", { start: startItem.toString(), end: endItem.toString(), total: totalItems.toString() })}
      </div>

      {/* Page navigation */}
      <div className="flex items-center gap-1">
        <IconButton
          size="sm"
          onClick={() => goToPage(currentPage - 1)}
          disabled={currentPage <= 1}
          title={t("common.previousPage")}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </IconButton>

        {getPageNumbers().map((page, index) => (
          <span key={index}>
            {page === "..." ? (
              <span className="px-3 py-2 text-sm text-muted-foreground">...</span>
            ) : (
              <Button
                variant={currentPage === page ? "default" : "ghost"}
                size="sm"
                onClick={() => goToPage(page as number)}
                className="min-w-[2.5rem]"
              >
                {page}
              </Button>
            )}
          </span>
        ))}

        <IconButton
          size="sm"
          onClick={() => goToPage(currentPage + 1)}
          disabled={currentPage >= totalPages}
          title={t("common.nextPage")}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
        </IconButton>
      </div>
    </div>
  );
}

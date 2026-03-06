"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { Button } from "./Button";

interface CursorPaginationProps {
  hasNextPage: boolean;
  hasPrevPage: boolean;
  pageSize: number;
  currentCount: number;
  pageSizeOptions?: number[];
  nextCursor?: string | null;
}

export function CursorPagination({
  hasNextPage,
  hasPrevPage,
  pageSize,
  currentCount,
  pageSizeOptions = [20, 50, 100],
  nextCursor,
}: CursorPaginationProps) {
  const router = useRouter();
  const searchParams = useSearchParams();

  const goToNext = () => {
    if (!nextCursor) return;
    const params = new URLSearchParams(searchParams);
    params.set("cursor", nextCursor);
    router.push(`?${params.toString()}`);
  };

  const goToFirst = () => {
    const params = new URLSearchParams(searchParams);
    params.delete("cursor");
    router.push(`?${params.toString()}`);
  };

  const changePageSize = (size: number) => {
    const params = new URLSearchParams(searchParams);
    params.set("limit", size.toString());
    params.delete("cursor");
    router.push(`?${params.toString()}`);
  };

  return (
    <div className="flex flex-col sm:flex-row items-center justify-between gap-6 mt-8 pt-8 border-t border-line">
      {/* Page size selector */}
      <div className="flex items-center gap-3">
        <span className="text-sm text-muted">Show</span>
        <select
          value={pageSize.toString()}
          onChange={(e) => changePageSize(Number(e.target.value))}
          className="w-20 px-3 py-2 text-sm border border-line rounded-lg bg-background text-foreground"
        >
          {pageSizeOptions.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
        <span className="text-sm text-muted">per page</span>
      </div>

      {/* Count info */}
      <div className="text-sm text-muted">
        Showing {currentCount} items
      </div>

      {/* Navigation */}
      <div className="flex items-center gap-3">
        {hasPrevPage && (
          <Button
            variant="secondary"
            size="sm"
            onClick={goToFirst}
          >
            ← First
          </Button>
        )}

        <Button
          variant="secondary"
          size="sm"
          onClick={goToNext}
          disabled={!hasNextPage}
        >
          Next →
        </Button>
      </div>
    </div>
  );
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
  pageSizeOptions = [20, 50, 100],
}: OffsetPaginationProps) {
  const router = useRouter();
  const searchParams = useSearchParams();

  const goToPage = (page: number) => {
    const params = new URLSearchParams(searchParams);
    params.set("page", page.toString());
    router.push(`?${params.toString()}`);
  };

  const changePageSize = (size: number) => {
    const params = new URLSearchParams(searchParams);
    params.set("limit", size.toString());
    params.set("page", "1");
    router.push(`?${params.toString()}`);
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
    <div className="flex flex-col sm:flex-row items-center justify-between gap-6 mt-8 pt-8 border-t border-line">
      {/* Page size selector */}
      <div className="flex items-center gap-3">
        <span className="text-sm text-muted">Show</span>
        <select
          value={pageSize.toString()}
          onChange={(e) => changePageSize(Number(e.target.value))}
          className="w-20 px-3 py-2 text-sm border border-line rounded-lg bg-background text-foreground"
        >
          {pageSizeOptions.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
        <span className="text-sm text-muted">per page</span>
      </div>

      {/* Page info */}
      <div className="text-sm text-muted">
        {startItem}-{endItem} of {totalItems}
      </div>

      {/* Page navigation */}
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => goToPage(currentPage - 1)}
          disabled={currentPage <= 1}
        >
          ←
        </Button>

        {getPageNumbers().map((page, index) => (
          <span key={index}>
            {page === "..." ? (
              <span className="px-3 py-2 text-sm text-muted">...</span>
            ) : (
              <Button
                variant={currentPage === page ? "primary" : "ghost"}
                size="sm"
                onClick={() => goToPage(page as number)}
              >
                {page}
              </Button>
            )}
          </span>
        ))}

        <Button
          variant="ghost"
          size="sm"
          onClick={() => goToPage(currentPage + 1)}
          disabled={currentPage >= totalPages}
        >
          →
        </Button>
      </div>
    </div>
  );
}

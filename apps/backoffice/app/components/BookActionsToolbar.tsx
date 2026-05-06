"use client";

import nextDynamic from "next/dynamic";
import type { BookDto } from "@/lib/api";
import { ActionsMenu, ActionsMenuItem, ActionsMenuSection } from "./ui";
import { MarkBookReadButton } from "./MarkBookReadButton";
import { ConvertButton } from "./ConvertButton";
import { DeleteBookButton } from "./DeleteBookButton";
import { RefreshButton } from "./RefreshButton";
import type { QuickSearch } from "./ProwlarrSearchModal";
import { useTranslation } from "../../lib/i18n/context";

const EditBookForm = nextDynamic(
  () => import("./EditBookForm").then(m => m.EditBookForm)
);

const ProwlarrSearchModal = nextDynamic(
  () => import("./ProwlarrSearchModal").then(m => m.ProwlarrSearchModal)
);

const RefreshIcon = ({ spinning }: { spinning: boolean }) => (
  <svg
    className={`w-4 h-4 ${spinning ? "animate-spin" : ""}`}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={2}
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
    <path d="M21 3v5h-5" />
    <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
    <path d="M3 21v-5h5" />
  </svg>
);

export function BookActionsToolbar({ book }: { book: BookDto }) {
  const { t } = useTranslation();

  // Prowlarr search context for this book
  const searchSeries = book.series ?? book.title;
  const initialQuery = book.volume != null
    ? `"${searchSeries}" T${book.volume}`
    : `"${searchSeries}"`;
  const quickSearches: QuickSearch[] = [
    { label: book.volume != null ? `${searchSeries} T${book.volume}` : searchSeries, query: initialQuery },
    { label: searchSeries, query: `"${searchSeries}"` },
  ];
  const defaultExpectedVolumes = book.volume != null ? [book.volume] : undefined;

  return (
    <div className="flex flex-wrap items-center gap-3">
      <MarkBookReadButton bookId={book.id} currentStatus={book.reading_status} />
      <EditBookForm book={book} />
      <ActionsMenu label={t("common.more")}>
        <ActionsMenuSection label={t("actionsMenu.download")}>
          <ProwlarrSearchModal
            seriesName={searchSeries}
            libraryId={book.library_id}
            missingBooks={null}
            initialQuery={initialQuery}
            quickSearches={quickSearches}
            defaultExpectedVolumes={defaultExpectedVolumes}
          >
            {(open) => (
              <ActionsMenuItem icon="⬇️" onClick={open}>
                {t("prowlarr.searchButton")}
              </ActionsMenuItem>
            )}
          </ProwlarrSearchModal>
        </ActionsMenuSection>
        <ActionsMenuSection label={t("actionsMenu.actions")}>
          <RefreshButton target="book-detail">
            {(onClick, pending) => (
              <ActionsMenuItem
                icon={<RefreshIcon spinning={pending} />}
                onClick={onClick}
                disabled={pending}
                keepOpen={pending}
              >
                {pending ? t("common.refreshing") : t("common.refresh")}
              </ActionsMenuItem>
            )}
          </RefreshButton>
          {book.file_format === "cbr" && (
            <ConvertButton bookId={book.id}>
              {(action, isLoading) => (
                <ActionsMenuItem
                  icon="🔄"
                  onClick={() => { action(); }}
                  disabled={isLoading}
                  keepOpen={isLoading}
                >
                  {isLoading ? t("convert.converting") : t("convert.convertToCbz")}
                </ActionsMenuItem>
              )}
            </ConvertButton>
          )}
          <DeleteBookButton bookId={book.id} libraryId={book.library_id}>
            {(open) => (
              <ActionsMenuItem icon="🗑" variant="danger" onClick={open}>
                {t("bookDetail.delete")}
              </ActionsMenuItem>
            )}
          </DeleteBookButton>
        </ActionsMenuSection>
      </ActionsMenu>
    </div>
  );
}

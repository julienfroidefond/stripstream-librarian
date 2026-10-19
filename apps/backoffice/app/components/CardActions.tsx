"use client";

import nextDynamic from "next/dynamic";

import type { BookDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import { ActionsMenu, ActionsMenuItem, ActionsMenuSection, Icon } from "./ui";
import { iconActionClassName, iconActionGroupClassName } from "./ui/iconAction";
import { AddToReadingListModal } from "./AddToReadingListModal";
import { ConvertButton } from "./ConvertButton";
import { DeleteBookButton } from "./DeleteBookButton";
import { DeleteSeriesButton } from "./DeleteSeriesButton";
import { DownloadBookFileButton } from "./DownloadBookFileButton";
import { MarkBookReadButton } from "./MarkBookReadButton";
import { MarkSeriesReadButton } from "./MarkSeriesReadButton";
import { RefreshButton } from "./RefreshButton";
import { RefreshIcon } from "./RefreshIcon";
import type { QuickSearch } from "./ProwlarrSearchModal";

const MetadataSearchModal = nextDynamic(
  () => import("./MetadataSearchModal").then((m) => m.MetadataSearchModal)
);
const ReadingStatusModal = nextDynamic(
  () => import("./ReadingStatusModal").then((m) => m.ReadingStatusModal)
);
const ProwlarrSearchModal = nextDynamic(
  () => import("./ProwlarrSearchModal").then((m) => m.ProwlarrSearchModal)
);
const RenameSeriesBooksModal = nextDynamic(
  () => import("./RenameSeriesBooksModal").then((m) => m.RenameSeriesBooksModal)
);
const MergeSeriesButton = nextDynamic(
  () => import("./MergeSeriesButton").then((m) => m.MergeSeriesButton)
);
const EditBookForm = nextDynamic(
  () => import("./EditBookForm").then((m) => m.EditBookForm)
);

/** Cover = round pills over the artwork; inline = neutral squares in a list row. */
export type CardActionVariant = "cover" | "inline";

interface SeriesCardActionsProps {
  libraryId: string;
  seriesId: string;
  seriesName: string;
  bookCount: number;
  booksReadCount: number;
  hasActiveUser?: boolean;
  variant?: CardActionVariant;
}

/**
 * Self-contained action cluster for a series card/row: direct mark-read and
 * delete shortcuts plus a `⋯` menu exposing the rest of the series toolset.
 * Always visible (no hover gating) so it stays reachable on touch devices.
 */
export function SeriesCardActions({
  libraryId,
  seriesId,
  seriesName,
  bookCount,
  booksReadCount,
  hasActiveUser = true,
  variant = "cover",
}: SeriesCardActionsProps) {
  const { t } = useTranslation();
  const readVariant = variant === "cover" ? "overlay" : "inline";

  return (
    <div className={`flex items-center ${iconActionGroupClassName(variant)}`}>
      {hasActiveUser && (
        <MarkSeriesReadButton
          seriesId={seriesId}
          seriesName={seriesName}
          bookCount={bookCount}
          booksReadCount={booksReadCount}
          compact
          iconOnly
          variant={readVariant}
        />
      )}
      <DeleteSeriesButton seriesId={seriesId}>
        {(open) => (
          <button
            type="button"
            onClick={open}
            aria-label={t("seriesDetail.delete")}
            title={t("seriesDetail.delete")}
            className={iconActionClassName(variant, "danger")}
          >
            <Icon name="trash" size="sm" />
          </button>
        )}
      </DeleteSeriesButton>
      <ActionsMenu
        lazy
        variant={variant}
        label={t("common.moreActions")}
        closeLabel={t("common.close")}
        align="right"
      >
        <ActionsMenuSection label={t("actionsMenu.metadata")}>
          <MetadataSearchModal
            libraryId={libraryId}
            seriesName={seriesName}
            existingLink={null}
            initialMissing={null}
          >
            {(open) => (
              <ActionsMenuItem icon="🔎" onClick={open}>
                {t("metadata.searchButton")}
              </ActionsMenuItem>
            )}
          </MetadataSearchModal>
          <ReadingStatusModal
            libraryId={libraryId}
            seriesId={seriesId}
            seriesName={seriesName}
            readingStatusProvider={null}
            existingLink={null}
          >
            {(open) => (
              <ActionsMenuItem icon="🔗" onClick={open}>
                {t("readingStatus.button")}
              </ActionsMenuItem>
            )}
          </ReadingStatusModal>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.download")}>
          <ProwlarrSearchModal
            seriesName={seriesName}
            libraryId={libraryId}
            missingBooks={null}
          >
            {(open) => (
              <ActionsMenuItem icon="⬇️" onClick={open}>
                {t("prowlarr.searchButton")}
              </ActionsMenuItem>
            )}
          </ProwlarrSearchModal>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.files")}>
          <RenameSeriesBooksModal seriesId={seriesId} seriesName={seriesName}>
            {(open) => (
              <ActionsMenuItem icon="📝" onClick={open}>
                {t("rename.button")}
              </ActionsMenuItem>
            )}
          </RenameSeriesBooksModal>
          <MergeSeriesButton seriesId={seriesId} seriesName={seriesName}>
            {(open) => (
              <ActionsMenuItem icon="🔀" onClick={open}>
                {t("seriesDetail.merge")}
              </ActionsMenuItem>
            )}
          </MergeSeriesButton>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.readingLists")}>
          <AddToReadingListModal seriesId={seriesId} seriesName={seriesName}>
            {(open) => (
              <ActionsMenuItem icon="🔖" onClick={open}>
                {t("readingLists.addToList")}
              </ActionsMenuItem>
            )}
          </AddToReadingListModal>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.actions")}>
          <RefreshButton target="series-detail" seriesId={seriesId}>
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
          <DeleteSeriesButton seriesId={seriesId}>
            {(open) => (
              <ActionsMenuItem icon="🗑" variant="danger" onClick={open}>
                {t("seriesDetail.delete")}
              </ActionsMenuItem>
            )}
          </DeleteSeriesButton>
        </ActionsMenuSection>
      </ActionsMenu>
    </div>
  );
}

interface BookCardActionsProps {
  book: BookDto & { coverUrl?: string };
  hasActiveUser?: boolean;
  variant?: CardActionVariant;
}

/**
 * Self-contained action cluster for a book card/row: direct mark-read,
 * download and delete shortcuts plus a `⋯` menu with the rest of the book
 * toolset (edit, Prowlarr, refresh, convert). Always visible so it works on
 * touch.
 */
export function BookCardActions({
  book,
  hasActiveUser = true,
  variant = "cover",
}: BookCardActionsProps) {
  const { t } = useTranslation();
  const readVariant = variant === "cover" ? "overlay" : "inline";

  // Prowlarr search context for this book (mirrors BookActionsToolbar).
  const searchSeries = book.series ?? book.title;
  const initialQuery =
    book.volume != null ? `"${searchSeries}" T${book.volume}` : `"${searchSeries}"`;
  const quickSearches: QuickSearch[] = [
    {
      label: book.volume != null ? `${searchSeries} T${book.volume}` : searchSeries,
      query: initialQuery,
    },
    { label: searchSeries, query: `"${searchSeries}"` },
  ];
  const defaultExpectedVolumes = book.volume != null ? [book.volume] : undefined;

  return (
    <div className={`flex items-center ${iconActionGroupClassName(variant)}`}>
      {hasActiveUser && (
        <MarkBookReadButton
          bookId={book.id}
          currentStatus={book.reading_status}
          compact
          iconOnly
          variant={readVariant}
        />
      )}
      <DownloadBookFileButton bookId={book.id}>
        {({ href, download, onClick }) => (
          <a
            href={href}
            download={download}
            onClick={onClick}
            aria-label={t("bookDetail.downloadOriginal")}
            title={t("bookDetail.downloadOriginal")}
            className={iconActionClassName(variant)}
          >
            <Icon name="download" size="sm" />
          </a>
        )}
      </DownloadBookFileButton>
      <DeleteBookButton bookId={book.id} seriesId={book.series_id ?? undefined}>
        {(open) => (
          <button
            type="button"
            onClick={open}
            aria-label={t("bookDetail.delete")}
            title={t("bookDetail.delete")}
            className={iconActionClassName(variant, "danger")}
          >
            <Icon name="trash" size="sm" />
          </button>
        )}
      </DeleteBookButton>
      <ActionsMenu
        lazy
        variant={variant}
        label={t("common.moreActions")}
        closeLabel={t("common.close")}
        align="right"
      >
        <ActionsMenuSection label={t("actionsMenu.actions")}>
          <DownloadBookFileButton bookId={book.id} />
          <EditBookForm book={book}>
            {(open) => (
              <ActionsMenuItem icon="✏️" onClick={open}>
                {t("common.edit")}
              </ActionsMenuItem>
            )}
          </EditBookForm>
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
                  onClick={() => {
                    action();
                  }}
                  disabled={isLoading}
                  keepOpen={isLoading}
                >
                  {isLoading ? t("convert.converting") : t("convert.convertToCbz")}
                </ActionsMenuItem>
              )}
            </ConvertButton>
          )}
          <DeleteBookButton bookId={book.id} seriesId={book.series_id ?? undefined}>
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

"use client";

import nextDynamic from "next/dynamic";
import type { ExternalMetadataLinkDto, MissingBooksDto, AnilistSeriesLinkDto } from "@/lib/api";
import { ActionsMenu, ActionsMenuItem, ActionsMenuSection } from "./ui";
import { MarkSeriesReadButton } from "./MarkSeriesReadButton";
import { RefreshButton } from "./RefreshButton";
import { useTranslation } from "../../lib/i18n/context";
import { AddToReadingListModal } from "./AddToReadingListModal";

const EditSeriesForm = nextDynamic(
  () => import("./EditSeriesForm").then(m => m.EditSeriesForm)
);
const MetadataSearchModal = nextDynamic(
  () => import("./MetadataSearchModal").then(m => m.MetadataSearchModal)
);
const ReadingStatusModal = nextDynamic(
  () => import("./ReadingStatusModal").then(m => m.ReadingStatusModal)
);
const ProwlarrSearchModal = nextDynamic(
  () => import("./ProwlarrSearchModal").then(m => m.ProwlarrSearchModal)
);
const DeleteSeriesButton = nextDynamic(
  () => import("./DeleteSeriesButton").then(m => m.DeleteSeriesButton)
);
const MergeSeriesButton = nextDynamic(
  () => import("./MergeSeriesButton").then(m => m.MergeSeriesButton)
);
const RenameSeriesBooksModal = nextDynamic(
  () => import("./RenameSeriesBooksModal").then(m => m.RenameSeriesBooksModal)
);

interface Props {
  libraryId: string;
  seriesId: string;
  seriesName: string;
  bookCount: number;
  booksReadCount: number;
  // Edit data
  editAuthors: string[];
  editGenres: string[];
  editPublishers: string[];
  editBookAuthor: string | null;
  editBookLanguage: string | null;
  editDescription: string | null;
  editStartYear: number | null;
  editTotalVolumes: number | null;
  editStatus: string | null;
  editLockedFields: Record<string, boolean>;
  // Metadata
  existingLink: ExternalMetadataLinkDto | null;
  missingData: MissingBooksDto | null;
  hiddenProviders: string[];
  // Reading status
  readingStatusProvider: string | null;
  readingStatusLink: AnilistSeriesLinkDto | null;
  // Prowlarr/qb
  prowlarrConfigured: boolean;
  qbConfigured: boolean;
  // Rename
  renameFormat: string | null;
  renameFormatHs: string | null;
}

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

export function SeriesActionsToolbar(props: Props) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-wrap items-center gap-3">
      <MarkSeriesReadButton
        seriesId={props.seriesId}
        seriesName={props.seriesName}
        bookCount={props.bookCount}
        booksReadCount={props.booksReadCount}
      />
      <EditSeriesForm
        libraryId={props.libraryId}
        seriesId={props.seriesId}
        seriesName={props.seriesName}
        currentAuthors={props.editAuthors}
        currentGenres={props.editGenres}
        currentPublishers={props.editPublishers}
        currentBookAuthor={props.editBookAuthor}
        currentBookLanguage={props.editBookLanguage}
        currentDescription={props.editDescription}
        currentStartYear={props.editStartYear}
        currentTotalVolumes={props.editTotalVolumes}
        currentStatus={props.editStatus}
        currentLockedFields={props.editLockedFields}
      />
      <ActionsMenu label={t("common.more")}>
        <ActionsMenuSection label={t("actionsMenu.metadata")}>
          <MetadataSearchModal
            libraryId={props.libraryId}
            seriesName={props.seriesName}
            existingLink={props.existingLink}
            initialMissing={props.missingData}
            initialHiddenProviders={props.hiddenProviders}
          >
            {(open) => (
              <ActionsMenuItem icon="🔎" onClick={open}>
                {props.existingLink && props.existingLink.status === "approved"
                  ? t("metadata.metadataButton")
                  : t("metadata.searchButton")}
              </ActionsMenuItem>
            )}
          </MetadataSearchModal>
          <ReadingStatusModal
            libraryId={props.libraryId}
            seriesId={props.seriesId}
            seriesName={props.seriesName}
            readingStatusProvider={props.readingStatusProvider}
            existingLink={props.readingStatusLink}
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
            seriesName={props.seriesName}
            libraryId={props.libraryId}
            missingBooks={props.missingData?.missing_books ?? null}
            initialProwlarrConfigured={props.prowlarrConfigured}
            initialQbConfigured={props.qbConfigured}
          >
            {(open) => (
              <ActionsMenuItem icon="⬇️" onClick={open}>
                {t("prowlarr.searchButton")}
              </ActionsMenuItem>
            )}
          </ProwlarrSearchModal>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.files")}>
          <RenameSeriesBooksModal
            seriesId={props.seriesId}
            seriesName={props.seriesName}
            initialFormat={props.renameFormat}
            initialFormatHs={props.renameFormatHs}
          >
            {(open) => (
              <ActionsMenuItem icon="📝" onClick={open}>
                {t("rename.button")}
              </ActionsMenuItem>
            )}
          </RenameSeriesBooksModal>
          <MergeSeriesButton
            seriesId={props.seriesId}
            seriesName={props.seriesName}
            libraryId={props.libraryId}
          >
            {(open) => (
              <ActionsMenuItem icon="🔀" onClick={open}>
                {t("seriesDetail.merge")}
              </ActionsMenuItem>
            )}
          </MergeSeriesButton>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("nav.readingLists")}>
          <AddToReadingListModal seriesId={props.seriesId} seriesName={props.seriesName}>
            {(open) => (
              <ActionsMenuItem icon="🔖" onClick={open} keepOpen>
                {t("readingLists.addToList")}
              </ActionsMenuItem>
            )}
          </AddToReadingListModal>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.actions")}>
          <RefreshButton target="series-detail" seriesId={props.seriesId}>
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
          <DeleteSeriesButton seriesId={props.seriesId}>
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

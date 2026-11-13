"use client";

import nextDynamic from "next/dynamic";
import type { SeriesActionsModel } from "@/lib/series/detail";
import { ActionsMenu, ActionsMenuItem, ActionsMenuSection, Icon } from "./ui";
import { MarkSeriesReadButton } from "./MarkSeriesReadButton";
import { RefreshButton } from "./RefreshButton";
import { RefreshIcon } from "./RefreshIcon";
import { useTranslation } from "@/lib/i18n/context";
import { AddToReadingListModal } from "./AddToReadingListModal";
import { SeriesShelfAction } from "./SeriesShelfAction";

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
const TelegramSearchModal = nextDynamic(
  () => import("./TelegramSearchModal").then(m => m.TelegramSearchModal)
);
const DdlProviderSearchModal = nextDynamic(
  () => import("./DdlProviderSearchModal").then(m => m.DdlProviderSearchModal)
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
  /** Résolu une seule fois par `loadSeriesDetail`. */
  model: SeriesActionsModel;
}

export function SeriesActionsToolbar({ model }: Props) {
  const { t } = useTranslation();
  const { metadata, renameFormats } = model;

  return (
    <div className="flex flex-wrap items-center gap-3">
      {model.hasActiveUser && (
        <MarkSeriesReadButton
          seriesId={model.seriesId}
          seriesName={model.seriesName}
          bookCount={model.bookCount}
          booksReadCount={model.booksReadCount}
        />
      )}
      <EditSeriesForm
        libraryId={model.libraryId}
        seriesId={model.seriesId}
        seriesName={model.seriesName}
        currentAuthors={metadata?.authors ?? []}
        currentGenres={metadata?.genres ?? []}
        currentPublishers={metadata?.publishers ?? []}
        currentBookAuthor={model.editBookAuthor}
        currentBookLanguage={model.editBookLanguage}
        currentDescription={metadata?.description ?? null}
        currentStartYear={metadata?.start_year ?? null}
        currentTotalVolumes={metadata?.total_volumes ?? null}
        currentStatus={metadata?.status ?? null}
        currentLockedFields={metadata?.locked_fields ?? {}}
      />
      <RefreshButton target="series-detail" seriesId={model.seriesId} />
      <ActionsMenu label={t("common.more")}>
        <ActionsMenuSection label={t("actionsMenu.metadata")}>
          <MetadataSearchModal
            libraryId={model.libraryId}
            seriesName={model.seriesName}
            existingLink={model.existingLink}
            links={model.metadataLinks}
            initialMissing={model.missingData}
            initialHiddenProviders={model.hiddenProviders}
          >
            {(open) => (
              <ActionsMenuItem icon="🔎" onClick={open}>
                {model.existingLink && model.existingLink.status === "approved"
                  ? t("metadata.metadataButton")
                  : t("metadata.searchButton")}
              </ActionsMenuItem>
            )}
          </MetadataSearchModal>
          <ReadingStatusModal
            libraryId={model.libraryId}
            seriesId={model.seriesId}
            seriesName={model.seriesName}
            readingStatusProvider={model.readingStatusProvider}
            existingLink={model.readingStatusLink}
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
            seriesName={model.seriesName}
            libraryId={model.libraryId}
            missingBooks={model.missingData?.missing_books ?? null}
            initialReadiness={model.readiness}
          >
            {(open) => (
              <ActionsMenuItem icon="⬇️" onClick={open}>
                {t("prowlarr.searchButton")}
              </ActionsMenuItem>
            )}
          </ProwlarrSearchModal>
          {model.readiness.telegram && (
            <TelegramSearchModal
              seriesName={model.seriesName}
              missingBooks={model.missingData?.missing_books ?? null}
              ownedVolumes={model.ownedVolumes ?? undefined}
              initialEnabled={model.readiness.telegram}
            >
              {(open) => (
                <ActionsMenuItem icon={<Icon name="send" size="sm" className="text-info" />} onClick={open}>
                  {t("telegramMonitor.searchButton")}
                </ActionsMenuItem>
              )}
            </TelegramSearchModal>
          )}
          {model.readiness.ddl && (
            <DdlProviderSearchModal
              seriesName={model.seriesName}
              libraryId={model.libraryId}
              missingBooks={model.missingData?.missing_books ?? null}
              initialEnabled={model.readiness.ddl}
            >
              {(open) => (
                <ActionsMenuItem icon={<Icon name="globe" size="sm" className="text-success" />} onClick={open}>
                  {t("ddlProviders.searchButton")}
                </ActionsMenuItem>
              )}
            </DdlProviderSearchModal>
          )}
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.files")}>
          <RenameSeriesBooksModal
            seriesId={model.seriesId}
            seriesName={model.seriesName}
            initialFormat={renameFormats.regular}
            initialFormatHs={renameFormats.hs}
            initialFormatInt={renameFormats.integral}
            initialFormatOneshot={renameFormats.oneshot}
          >
            {(open) => (
              <ActionsMenuItem icon="📝" onClick={open}>
                {t("rename.button")}
              </ActionsMenuItem>
            )}
          </RenameSeriesBooksModal>
          <MergeSeriesButton
            seriesId={model.seriesId}
            seriesName={model.seriesName}
          >
            {(open) => (
              <ActionsMenuItem icon="🔀" onClick={open}>
                {t("seriesDetail.merge")}
              </ActionsMenuItem>
            )}
          </MergeSeriesButton>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.readingLists")}>
          <AddToReadingListModal seriesId={model.seriesId} seriesName={model.seriesName}>
            {(open) => (
              <ActionsMenuItem icon="🔖" onClick={open}>
                {t("readingLists.addToList")}
              </ActionsMenuItem>
            )}
          </AddToReadingListModal>
        </ActionsMenuSection>

        <ActionsMenuSection label={t("actionsMenu.actions")}>
          {model.hasActiveUser && (
            <SeriesShelfAction
              seriesId={model.seriesId}
              initialIsShelved={model.isShelved}
            />
          )}
          <RefreshButton target="series-detail" seriesId={model.seriesId}>
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
          <DeleteSeriesButton seriesId={model.seriesId}>
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

"use client";

import nextDynamic from "next/dynamic";
import { ActionsMenu, ActionsMenuItem, ActionsMenuSection, Icon } from "./ui";
import type { AcquisitionReadiness } from "@/lib/acquisitionReadiness";
import type { QuickSearch } from "./ProwlarrSearchModal";
import { useTranslation } from "@/lib/i18n/context";

const ProwlarrSearchModal = nextDynamic(
  () => import("./ProwlarrSearchModal").then(m => m.ProwlarrSearchModal)
);

const TelegramSearchModal = nextDynamic(
  () => import("./TelegramSearchModal").then(m => m.TelegramSearchModal)
);

const DdlProviderSearchModal = nextDynamic(
  () => import("./DdlProviderSearchModal").then(m => m.DdlProviderSearchModal)
);

interface MissingBookItem {
  title: string | null;
  volume_number: number | null;
  external_book_id: string | null;
}

interface Props {
  seriesName: string;
  libraryId: string;
  volumeNumber: number | null;
  missingBook: MissingBookItem;
  readiness: AcquisitionReadiness;
}

export function MissingBookActionsToolbar({
  seriesName,
  libraryId,
  volumeNumber,
  missingBook,
  readiness,
}: Props) {
  const { t } = useTranslation();

  const initialQuery = volumeNumber != null
    ? `"${seriesName}" T${volumeNumber}`
    : `"${seriesName}"`;
  const quickSearches: QuickSearch[] = [
    { label: volumeNumber != null ? `${seriesName} T${volumeNumber}` : seriesName, query: initialQuery },
    { label: seriesName, query: `"${seriesName}"` },
  ];

  return (
    <ActionsMenu label={t("common.more")}>
      <ActionsMenuSection label={t("actionsMenu.download")}>
        <ProwlarrSearchModal
          seriesName={seriesName}
          libraryId={libraryId}
          missingBooks={[missingBook]}
          initialReadiness={readiness}
          initialQuery={initialQuery}
          quickSearches={quickSearches}
          defaultExpectedVolumes={volumeNumber != null ? [volumeNumber] : undefined}
        >
          {(open) => (
            <ActionsMenuItem icon="⬇️" onClick={open}>
              {t("prowlarr.searchButton")}
            </ActionsMenuItem>
          )}
        </ProwlarrSearchModal>
        {readiness.telegram && (
          <TelegramSearchModal
            seriesName={seriesName}
            missingBooks={[missingBook]}
            initialEnabled={readiness.telegram}
            initialQuery={initialQuery}
          >
            {(open) => (
              <ActionsMenuItem icon={<Icon name="send" size="sm" className="text-info" />} onClick={open}>
                {t("telegramMonitor.searchButton")}
              </ActionsMenuItem>
            )}
          </TelegramSearchModal>
        )}
        {readiness.ddl && (
          <DdlProviderSearchModal
            seriesName={seriesName}
            libraryId={libraryId}
            missingBooks={[missingBook]}
            targetVolume={volumeNumber}
            initialQuery={initialQuery}
            initialEnabled={readiness.ddl}
          >
            {(open) => (
              <ActionsMenuItem icon={<Icon name="globe" size="sm" className="text-success" />} onClick={open}>
                {t("ddlProviders.searchButton")}
              </ActionsMenuItem>
            )}
          </DdlProviderSearchModal>
        )}
      </ActionsMenuSection>
    </ActionsMenu>
  );
}

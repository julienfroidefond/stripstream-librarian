"use client";

import nextDynamic from "next/dynamic";
import { ActionsMenu, ActionsMenuItem, ActionsMenuSection, Icon } from "./ui";
import type { QuickSearch } from "./ProwlarrSearchModal";
import { useTranslation } from "@/lib/i18n/context";

const ProwlarrSearchModal = nextDynamic(
  () => import("./ProwlarrSearchModal").then(m => m.ProwlarrSearchModal)
);

const TelegramSearchModal = nextDynamic(
  () => import("./TelegramSearchModal").then(m => m.TelegramSearchModal)
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
  prowlarrConfigured: boolean;
  qbConfigured: boolean;
  telegramEnabled: boolean;
}

export function MissingBookActionsToolbar({
  seriesName,
  libraryId,
  volumeNumber,
  missingBook,
  prowlarrConfigured,
  qbConfigured,
  telegramEnabled,
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
          initialProwlarrConfigured={prowlarrConfigured}
          initialQbConfigured={qbConfigured}
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
        {telegramEnabled && (
          <TelegramSearchModal
            seriesName={seriesName}
            missingBooks={[missingBook]}
            initialEnabled={telegramEnabled}
            initialQuery={initialQuery}
          >
            {(open) => (
              <ActionsMenuItem icon={<Icon name="send" size="sm" className="text-sky-500" />} onClick={open}>
                {t("telegramMonitor.searchButton")}
              </ActionsMenuItem>
            )}
          </TelegramSearchModal>
        )}
      </ActionsMenuSection>
    </ActionsMenu>
  );
}

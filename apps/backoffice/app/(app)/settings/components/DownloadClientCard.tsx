"use client";

import { SettingsCard } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n/fr";
import {
  DOWNLOAD_CLIENTS,
  DOWNLOAD_CLIENT_LABELS,
  DOWNLOAD_CLIENT_SETTING_KEY,
  isClientConfigured,
  readDownloadClientSetting,
  type DownloadClientSetting,
} from "@/lib/downloadClient";
import type { DownloadClient } from "@/lib/api";
import type { AlldebridSettings, QbittorrentSettings } from "@/lib/settingsSections";
import { useSettingsSection } from "../SettingsDraftProvider";

/** Per-client description line under its brand name. */
const CLIENT_DESCRIPTIONS: Record<DownloadClient, TranslationKey> = {
  qbittorrent: "settings.downloadClientQbittorrent",
  alldebrid: "settings.downloadClientAlldebrid",
};

/**
 * The single place where the download client is chosen.
 *
 * qBittorrent and AllDebrid are alternatives, never cumulative: every snatch
 * goes to the selected one. The choice used to be offered in the modals (and
 * kept in `localStorage`); it is now an admin setting so nothing else in the UI
 * has to ask again.
 *
 * Readiness is derived from the same draft sections the two cards below edit,
 * so enabling AllDebrid immediately enables the radio without a callback.
 */
export function DownloadClientCard() {
  const { t } = useTranslation();
  const section = useSettingsSection<DownloadClientSetting>(DOWNLOAD_CLIENT_SETTING_KEY);
  const qbittorrent = useSettingsSection<QbittorrentSettings>("qbittorrent");
  const alldebrid = useSettingsSection<AlldebridSettings>("alldebrid");
  const configured: Record<DownloadClient, boolean> = {
    qbittorrent: isClientConfigured("qbittorrent", qbittorrent.value as unknown as Record<string, unknown>),
    alldebrid: isClientConfigured("alldebrid", alldebrid.value as unknown as Record<string, unknown>),
  };
  // Never render a client that is not currently usable: this mirrors the
  // runtime fallback exactly (see `DownloadClientProvider`). The stored choice
  // lives in the draft, so a rollback also reverts the selection here.
  const stored = readDownloadClientSetting(section.value);
  const client: DownloadClient | null =
    stored && configured[stored]
      ? stored
      : (DOWNLOAD_CLIENTS.find((c) => configured[c]) ?? null);

  function choose(next: DownloadClient) {
    if (!configured[next]) return;
    section.set({ client: next });
    void section.save();
  }

  return (
    <SettingsCard
      icon="download"
      title={t("settings.downloadClient")}
      description={t("settings.downloadClientDesc")}
    >
      <div role="radiogroup" aria-label={t("settings.downloadClient")} className="space-y-2">
        {DOWNLOAD_CLIENTS.map((c) => {
          const isConfigured = configured[c];
          const selected = client === c;
          return (
            <label
              key={c}
              className={`flex items-start gap-3 rounded-xl border p-3.5 transition-colors ${
                isConfigured ? "cursor-pointer" : "cursor-not-allowed opacity-60"
              } ${selected ? "border-primary/40 bg-primary/5" : "border-border hover:bg-accent/40"}`}
            >
              <input
                type="radio"
                name="download_client"
                value={c}
                checked={selected}
                disabled={!isConfigured}
                onChange={() => choose(c)}
                className="mt-0.5 h-4 w-4 shrink-0 accent-primary"
              />
              <span className="min-w-0">
                <span className="block text-sm font-medium text-foreground">{DOWNLOAD_CLIENT_LABELS[c]}</span>
                <span className="block text-xs leading-relaxed text-muted-foreground">
                  {isConfigured ? t(CLIENT_DESCRIPTIONS[c]) : t("settings.downloadClientNotConfigured")}
                </span>
              </span>
            </label>
          );
        })}
      </div>
    </SettingsCard>
  );
}

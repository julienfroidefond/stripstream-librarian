"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, FormSelect, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { TestConnectionButton } from "./TestConnectionButton";

export function QBittorrentCard({ handleUpdateSetting, initialQbittorrent, initialTorrentImport }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void>; initialQbittorrent: Record<string, unknown> | null; initialTorrentImport: Record<string, unknown> | null }) {
  const { t } = useTranslation();
  const [qbUrl, setQbUrl] = useState(initialQbittorrent?.url ? String(initialQbittorrent.url) : "");
  const [qbUsername, setQbUsername] = useState(initialQbittorrent?.username ? String(initialQbittorrent.username) : "");
  const [qbPassword, setQbPassword] = useState(initialQbittorrent?.password ? String(initialQbittorrent.password) : "");
  const [importEnabled, setImportEnabled] = useState(initialTorrentImport?.enabled === true);

  function saveQbittorrent() {
    handleUpdateSetting("qbittorrent", {
      url: qbUrl,
      username: qbUsername,
      password: qbPassword,
    });
  }

  return (
    <SettingsCard icon="settings" title={t("settings.qbittorrent")} description={t("settings.qbittorrentDesc")}>
      <SettingsRow label={t("settings.qbittorrentUrl")}>
        <FormInput
          type="url"
          placeholder={t("settings.qbittorrentUrlPlaceholder")}
          value={qbUrl}
          onChange={(e) => setQbUrl(e.target.value)}
          onBlur={() => saveQbittorrent()}
        />
      </SettingsRow>
      <SettingsRow label={t("settings.qbittorrentUsername")}>
        <FormInput
          type="text"
          value={qbUsername}
          onChange={(e) => setQbUsername(e.target.value)}
          onBlur={() => saveQbittorrent()}
        />
      </SettingsRow>
      <SettingsRow label={t("settings.qbittorrentPassword")}>
        <FormInput
          type="password" autoComplete="off"
          value={qbPassword}
          onChange={(e) => setQbPassword(e.target.value)}
          onBlur={() => saveQbittorrent()}
        />
      </SettingsRow>

      <div className="pt-1">
        <TestConnectionButton
          endpoint="/api/qbittorrent/test"
          disabled={!qbUrl || !qbUsername}
        />
      </div>

      <div className="space-y-3 pt-2">
        <SettingsRow label={t("settings.torrentImportEnabled")}>
          <FormSelect
            value={importEnabled ? "true" : "false"}
            onChange={(e) => {
              const val = e.target.value === "true";
              setImportEnabled(val);
              handleUpdateSetting("torrent_import", { enabled: val });
            }}
            className="w-40"
          >
            <option value="false">{t("common.disabled")}</option>
            <option value="true">{t("common.enabled")}</option>
          </FormSelect>
        </SettingsRow>

        {importEnabled && (
          <div className="flex items-start gap-2 rounded-xl bg-success/10 p-3">
            <Icon name="check" size="sm" mono className="mt-0.5 shrink-0 text-success" />
            <p className="text-sm text-muted-foreground">{t("settings.torrentImportPollingInfo")}</p>
          </div>
        )}
      </div>
    </SettingsCard>
  );
}

"use client";

import { useEffect, useState } from "react";
import { SettingsCard, SettingsRow, FormInput, Icon, Switch } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { isClientConfigured } from "@/lib/downloadClient";
import { TestConnectionButton } from "./TestConnectionButton";

export function QBittorrentCard({ handleUpdateSetting, initialQbittorrent, initialTorrentImport, onConfiguredChange }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void>; initialQbittorrent: Record<string, unknown> | null; initialTorrentImport: Record<string, unknown> | null; onConfiguredChange?: (configured: boolean) => void }) {
  const { t } = useTranslation();
  const [qbUrl, setQbUrl] = useState(initialQbittorrent?.url ? String(initialQbittorrent.url) : "");
  const [qbUsername, setQbUsername] = useState(initialQbittorrent?.username ? String(initialQbittorrent.username) : "");
  const [qbPassword, setQbPassword] = useState(initialQbittorrent?.password ? String(initialQbittorrent.password) : "");
  const [enabled, setEnabled] = useState(initialQbittorrent?.enabled !== false);
  const [importEnabled, setImportEnabled] = useState(initialTorrentImport?.enabled === true);

  // Keep the parent (download-client picker) in sync with the local edits.
  useEffect(() => {
    onConfiguredChange?.(
      isClientConfigured("qbittorrent", { url: qbUrl, username: qbUsername, password: qbPassword, enabled })
    );
  }, [qbUrl, qbUsername, qbPassword, enabled, onConfiguredChange]);

  function saveQbittorrent(overrides: Record<string, unknown> = {}) {
    handleUpdateSetting("qbittorrent", {
      url: qbUrl,
      username: qbUsername,
      password: qbPassword,
      enabled,
      ...overrides,
    });
  }

  return (
    <SettingsCard
      icon="settings"
      title={t("settings.qbittorrent")}
      description={t("settings.qbittorrentDesc")}
      action={
        <Switch
          checked={enabled}
          aria-label={t("settings.qbittorrentEnabled")}
          onChange={(e) => {
            const val = e.target.checked;
            setEnabled(val);
            saveQbittorrent({ enabled: val });
          }}
        />
      }
    >
      <SettingsRow label={t("settings.qbittorrentUrl")}>
        <FormInput
          type="url"
          placeholder={t("settings.qbittorrentUrlPlaceholder")}
          value={qbUrl}
          onChange={(e) => setQbUrl(e.target.value)}
          onBlur={() => saveQbittorrent()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>
      <SettingsRow label={t("settings.qbittorrentUsername")}>
        <FormInput
          type="text"
          value={qbUsername}
          onChange={(e) => setQbUsername(e.target.value)}
          onBlur={() => saveQbittorrent()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>
      <SettingsRow label={t("settings.qbittorrentPassword")}>
        <FormInput
          type="password"
          value={qbPassword}
          onChange={(e) => setQbPassword(e.target.value)}
          onBlur={() => saveQbittorrent()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>

      <div className="pt-1">
        <TestConnectionButton
          endpoint="/api/qbittorrent/test"
          disabled={!enabled || !qbUrl || !qbUsername}
        />
      </div>

      <div className="space-y-3 pt-2">
        <SettingsRow label={t("settings.torrentImportEnabled")}>
          <Switch
            checked={importEnabled}
            aria-label={t("settings.torrentImportEnabled")}
            onChange={(e) => {
              const val = e.target.checked;
              setImportEnabled(val);
              handleUpdateSetting("torrent_import", { enabled: val });
            }}
          />
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

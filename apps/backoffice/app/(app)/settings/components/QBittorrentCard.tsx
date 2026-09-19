"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormInput, FormSelect, Icon } from "@/app/components/ui";
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
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="settings" size="md" />
          {t("settings.qbittorrent")}
        </CardTitle>
        <CardDescription>{t("settings.qbittorrentDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.qbittorrentUrl")}</label>
              <FormInput
                type="url"
                placeholder={t("settings.qbittorrentUrlPlaceholder")}
                value={qbUrl}
                onChange={(e) => setQbUrl(e.target.value)}
                onBlur={() => saveQbittorrent()}
              />
            </FormField>
          </div>
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.qbittorrentUsername")}</label>
              <FormInput
                type="text"
                value={qbUsername}
                onChange={(e) => setQbUsername(e.target.value)}
                onBlur={() => saveQbittorrent()}
              />
            </FormField>
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.qbittorrentPassword")}</label>
              <FormInput
                type="password" autoComplete="off"
                value={qbPassword}
                onChange={(e) => setQbPassword(e.target.value)}
                onBlur={() => saveQbittorrent()}
              />
            </FormField>
          </div>

          <div className="flex items-center gap-3">
            <TestConnectionButton
              endpoint="/api/qbittorrent/test"
              disabled={!qbUrl || !qbUsername}
            />
          </div>

          <div className="border-t border-border/40 pt-4">
            <FormField className="max-w-xs">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">
                {t("settings.torrentImportEnabled")}
              </label>
              <FormSelect
                value={importEnabled ? "true" : "false"}
                onChange={(e) => {
                  const val = e.target.value === "true";
                  setImportEnabled(val);
                  handleUpdateSetting("torrent_import", { enabled: val });
                }}
              >
                <option value="false">{t("common.disabled")}</option>
                <option value="true">{t("common.enabled")}</option>
              </FormSelect>
            </FormField>

            {importEnabled && (
              <div className="mt-3 rounded-lg border border-success/20 bg-success/5 p-3 flex items-start gap-2">
                <Icon name="check" size="sm" className="text-success mt-0.5 shrink-0" />
                <p className="text-sm text-muted-foreground">{t("settings.torrentImportPollingInfo")}</p>
              </div>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

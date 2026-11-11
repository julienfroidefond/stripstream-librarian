"use client";

import { SettingsCard, SettingsRow, FormInput, Switch } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { QbittorrentSettings } from "@/lib/settingsSections";
import { useSettingsSection } from "../SettingsDraftProvider";
import { TestConnectionButton } from "./TestConnectionButton";

export function QBittorrentCard() {
  const { t } = useTranslation();
  const section = useSettingsSection<QbittorrentSettings>("qbittorrent");
  const value = section.value;

  return (
    <SettingsCard
      icon="settings"
      title={t("settings.qbittorrent")}
      description={t("settings.qbittorrentDesc")}
      action={
        <Switch
          checked={value.enabled}
          aria-label={t("settings.qbittorrentEnabled")}
          onChange={(e) => {
            section.patch({ enabled: e.target.checked });
            void section.save();
          }}
        />
      }
    >
      <SettingsRow label={t("settings.qbittorrentUrl")}>
        <FormInput
          type="url"
          placeholder={t("settings.qbittorrentUrlPlaceholder")}
          value={value.url}
          onChange={(e) => section.patch({ url: e.target.value })}
          onBlur={() => void section.save()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>
      <SettingsRow label={t("settings.qbittorrentUsername")}>
        <FormInput
          type="text"
          value={value.username}
          onChange={(e) => section.patch({ username: e.target.value })}
          onBlur={() => void section.save()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>
      <SettingsRow label={t("settings.qbittorrentPassword")}>
        <FormInput
          type="password"
          value={value.password}
          onChange={(e) => section.patch({ password: e.target.value })}
          onBlur={() => void section.save()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>

      <div className="pt-1">
        <TestConnectionButton
          endpoint="/api/qbittorrent/test"
          disabled={!value.enabled || !value.url || !value.username}
        />
      </div>
    </SettingsCard>
  );
}

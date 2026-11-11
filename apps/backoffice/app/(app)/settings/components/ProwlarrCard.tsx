"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, FormSelect, Switch } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { ProwlarrSettings } from "@/lib/settingsSections";
import { useSettingsDraft, useSettingsSection } from "../SettingsDraftProvider";
import { TestConnectionButton } from "./TestConnectionButton";

export function ProwlarrCard() {
  const { t } = useTranslation();
  const draft = useSettingsDraft();
  const section = useSettingsSection<ProwlarrSettings>("prowlarr");
  const value = section.value;

  const categoriesKey = (Array.isArray(value.categories) ? value.categories : []).join(", ");
  // Raw text so a category list can be typed freely; re-synced from the
  // acknowledged section when a save normalizes or a rollback restores it.
  const [categoriesText, setCategoriesText] = useState(categoriesKey);

  async function commitCategories(raw: string) {
    const categories = raw
      .split(",")
      .map((s) => parseInt(s.trim()))
      .filter((n) => !isNaN(n));
    section.patch({ categories });
    const result = await section.save();
    if (result.status !== "unchanged") {
      const stored = draft.get<ProwlarrSettings>("prowlarr");
      setCategoriesText((stored?.categories ?? []).join(", "));
    }
  }

  const RSS_INTERVAL_OPTIONS = [
    { value: "0",     label: t("settings.prowlarrRssIntervalDisabled") },
    { value: "30",    label: t("settings.prowlarrRssInterval30m") },
    { value: "60",    label: t("settings.prowlarrRssInterval1h") },
    { value: "360",   label: t("settings.prowlarrRssInterval6h") },
    { value: "720",   label: t("settings.prowlarrRssInterval12h") },
    { value: "1440",  label: t("settings.prowlarrRssInterval1d") },
    { value: "10080", label: t("settings.prowlarrRssInterval1w") },
  ];

  return (
    <SettingsCard
      icon="search"
      title={t("settings.prowlarr")}
      description={t("settings.prowlarrDesc")}
      action={
        <Switch
          checked={value.enabled}
          aria-label={t("settings.prowlarrEnabled")}
          onChange={(e) => {
            section.patch({ enabled: e.target.checked });
            void section.save();
          }}
        />
      }
    >
      <SettingsRow label={t("settings.prowlarrUrl")}>
        <FormInput
          type="url"
          placeholder={t("settings.prowlarrUrlPlaceholder")}
          value={value.url}
          onChange={(e) => section.patch({ url: e.target.value })}
          onBlur={() => void section.save()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>
      <SettingsRow label={t("settings.prowlarrApiKey")}>
        <FormInput
          type="password"
          placeholder={t("settings.prowlarrApiKeyPlaceholder")}
          value={value.api_key}
          onChange={(e) => section.patch({ api_key: e.target.value })}
          onBlur={() => void section.save()}
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
        />
      </SettingsRow>
      <SettingsRow label={t("settings.prowlarrCategories")} description={t("settings.prowlarrCategoriesHelp")}>
        <FormInput
          type="text"
          placeholder="7030, 7020"
          value={categoriesText}
          onChange={(e) => setCategoriesText(e.target.value)}
          onBlur={() => commitCategories(categoriesText)}
        />
      </SettingsRow>
      <SettingsRow label={t("settings.prowlarrRssInterval")} description={t("settings.prowlarrRssIntervalHelp")}>
        <FormSelect
          value={String(value.rss_poll_interval_minutes)}
          onChange={(e) => {
            section.patch({ rss_poll_interval_minutes: parseInt(e.target.value) || 30 });
            void section.save();
          }}
        >
          {RSS_INTERVAL_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </FormSelect>
      </SettingsRow>
      <div className="pt-1">
        <TestConnectionButton
          endpoint="/api/prowlarr/test"
          disabled={!value.enabled || !value.url || !value.api_key}
        />
      </div>
    </SettingsCard>
  );
}

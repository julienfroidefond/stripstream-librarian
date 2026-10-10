"use client";

import { useState } from "react";
import { SettingsCard, SettingsField, FormInput, FormSelect } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { TestConnectionButton } from "./TestConnectionButton";

export function ProwlarrCard({ handleUpdateSetting, initialData }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void>; initialData: Record<string, unknown> | null }) {
  const { t } = useTranslation();
  const [prowlarrUrl, setProwlarrUrl] = useState(initialData?.url ? String(initialData.url) : "");
  const [prowlarrApiKey, setProwlarrApiKey] = useState(initialData?.api_key ? String(initialData.api_key) : "");
  const [prowlarrCategories, setProwlarrCategories] = useState(
    Array.isArray(initialData?.categories) ? (initialData.categories as number[]).join(", ") : "7030, 7020"
  );
  const [rssPollInterval, setRssPollInterval] = useState(
    initialData?.rss_poll_interval_minutes != null ? String(initialData.rss_poll_interval_minutes) : "30"
  );

  const RSS_INTERVAL_OPTIONS = [
    { value: "0",     label: t("settings.prowlarrRssIntervalDisabled") },
    { value: "30",    label: t("settings.prowlarrRssInterval30m") },
    { value: "60",    label: t("settings.prowlarrRssInterval1h") },
    { value: "360",   label: t("settings.prowlarrRssInterval6h") },
    { value: "720",   label: t("settings.prowlarrRssInterval12h") },
    { value: "1440",  label: t("settings.prowlarrRssInterval1d") },
    { value: "10080", label: t("settings.prowlarrRssInterval1w") },
  ];
  function saveProwlarr(url?: string, apiKey?: string, cats?: string, interval?: string) {
    const categories = (cats ?? prowlarrCategories)
      .split(",")
      .map((s) => parseInt(s.trim()))
      .filter((n) => !isNaN(n));
    const parsedInterval = parseInt(interval ?? rssPollInterval);
    handleUpdateSetting("prowlarr", {
      url: url ?? prowlarrUrl,
      api_key: apiKey ?? prowlarrApiKey,
      categories,
      rss_poll_interval_minutes: isNaN(parsedInterval) ? 30 : parsedInterval,
    });
  }

  return (
    <SettingsCard icon="search" title={t("settings.prowlarr")} description={t("settings.prowlarrDesc")}>
      <div className="flex gap-4">
        <SettingsField className="flex-1" label={t("settings.prowlarrUrl")}>
          <FormInput
            type="url"
            placeholder={t("settings.prowlarrUrlPlaceholder")}
            value={prowlarrUrl}
            onChange={(e) => setProwlarrUrl(e.target.value)}
            onBlur={() => saveProwlarr()}
          />
        </SettingsField>
      </div>
      <div className="flex gap-4">
        <SettingsField className="flex-1" label={t("settings.prowlarrApiKey")}>
          <FormInput
            type="password" autoComplete="off"
            placeholder={t("settings.prowlarrApiKeyPlaceholder")}
            value={prowlarrApiKey}
            onChange={(e) => setProwlarrApiKey(e.target.value)}
            onBlur={() => saveProwlarr()}
          />
        </SettingsField>
      </div>
      <div className="flex gap-4">
        <SettingsField className="flex-1" label={t("settings.prowlarrCategories")} help={t("settings.prowlarrCategoriesHelp")}>
          <FormInput
            type="text"
            placeholder="7030, 7020"
            value={prowlarrCategories}
            onChange={(e) => setProwlarrCategories(e.target.value)}
            onBlur={() => saveProwlarr()}
          />
        </SettingsField>
      </div>
      <div className="flex gap-4">
        <SettingsField className="w-64" label={t("settings.prowlarrRssInterval")} help={t("settings.prowlarrRssIntervalHelp")}>
          <FormSelect
            value={rssPollInterval}
            onChange={(e) => { setRssPollInterval(e.target.value); saveProwlarr(undefined, undefined, undefined, e.target.value); }}
          >
            {RSS_INTERVAL_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </FormSelect>
        </SettingsField>
      </div>
      <div className="flex items-center gap-3">
        <TestConnectionButton
          endpoint="/api/prowlarr/test"
          disabled={!prowlarrUrl || !prowlarrApiKey}
        />
      </div>
    </SettingsCard>
  );
}

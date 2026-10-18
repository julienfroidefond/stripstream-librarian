"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, FormSelect, Switch } from "@/app/components/ui";
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
  const [enabled, setEnabled] = useState(initialData?.enabled !== false);

  const RSS_INTERVAL_OPTIONS = [
    { value: "0",     label: t("settings.prowlarrRssIntervalDisabled") },
    { value: "30",    label: t("settings.prowlarrRssInterval30m") },
    { value: "60",    label: t("settings.prowlarrRssInterval1h") },
    { value: "360",   label: t("settings.prowlarrRssInterval6h") },
    { value: "720",   label: t("settings.prowlarrRssInterval12h") },
    { value: "1440",  label: t("settings.prowlarrRssInterval1d") },
    { value: "10080", label: t("settings.prowlarrRssInterval1w") },
  ];
  function saveProwlarr(url?: string, apiKey?: string, cats?: string, interval?: string, en?: boolean) {
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
      enabled: en ?? enabled,
    });
  }

  return (
    <SettingsCard
      icon="search"
      title={t("settings.prowlarr")}
      description={t("settings.prowlarrDesc")}
      action={
        <Switch
          checked={enabled}
          aria-label={t("settings.prowlarrEnabled")}
          onChange={(e) => {
            const val = e.target.checked;
            setEnabled(val);
            saveProwlarr(undefined, undefined, undefined, undefined, val);
          }}
        />
      }
    >
      <SettingsRow label={t("settings.prowlarrUrl")}>
        <FormInput
          type="url"
          placeholder={t("settings.prowlarrUrlPlaceholder")}
          value={prowlarrUrl}
          onChange={(e) => setProwlarrUrl(e.target.value)}
          onBlur={() => saveProwlarr()}
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
          value={prowlarrApiKey}
          onChange={(e) => setProwlarrApiKey(e.target.value)}
          onBlur={() => saveProwlarr()}
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
          value={prowlarrCategories}
          onChange={(e) => setProwlarrCategories(e.target.value)}
          onBlur={() => saveProwlarr()}
        />
      </SettingsRow>
      <SettingsRow label={t("settings.prowlarrRssInterval")} description={t("settings.prowlarrRssIntervalHelp")}>
        <FormSelect
          value={rssPollInterval}
          onChange={(e) => { setRssPollInterval(e.target.value); saveProwlarr(undefined, undefined, undefined, e.target.value); }}
        >
          {RSS_INTERVAL_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </FormSelect>
      </SettingsRow>
      <div className="pt-1">
        <TestConnectionButton
          endpoint="/api/prowlarr/test"
          disabled={!enabled || !prowlarrUrl || !prowlarrApiKey}
        />
      </div>
    </SettingsCard>
  );
}

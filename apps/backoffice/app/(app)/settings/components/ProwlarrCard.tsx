"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

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
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

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

  async function handleTestConnection() {
    setIsTesting(true);
    setTestResult(null);
    try {
      const resp = await fetch("/api/prowlarr/test");
      const data = await resp.json();
      if (data.error) {
        setTestResult({ success: false, message: data.error });
      } else {
        setTestResult(data);
      }
    } catch {
      setTestResult({ success: false, message: "Failed to connect" });
    } finally {
      setIsTesting(false);
    }
  }

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="search" size="md" />
          {t("settings.prowlarr")}
        </CardTitle>
        <CardDescription>{t("settings.prowlarrDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.prowlarrUrl")}</label>
              <FormInput
                type="url"
                placeholder={t("settings.prowlarrUrlPlaceholder")}
                value={prowlarrUrl}
                onChange={(e) => setProwlarrUrl(e.target.value)}
                onBlur={() => saveProwlarr()}
              />
            </FormField>
          </div>
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.prowlarrApiKey")}</label>
              <FormInput
                type="password" autoComplete="off"
                placeholder={t("settings.prowlarrApiKeyPlaceholder")}
                value={prowlarrApiKey}
                onChange={(e) => setProwlarrApiKey(e.target.value)}
                onBlur={() => saveProwlarr()}
              />
            </FormField>
          </div>
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.prowlarrCategories")}</label>
              <FormInput
                type="text"
                placeholder="7030, 7020"
                value={prowlarrCategories}
                onChange={(e) => setProwlarrCategories(e.target.value)}
                onBlur={() => saveProwlarr()}
              />
              <p className="text-xs text-muted-foreground mt-1">{t("settings.prowlarrCategoriesHelp")}</p>
            </FormField>
          </div>
          <div className="flex gap-4">
            <FormField className="w-64">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.prowlarrRssInterval")}</label>
              <FormSelect
                value={rssPollInterval}
                onChange={(e) => { setRssPollInterval(e.target.value); saveProwlarr(undefined, undefined, undefined, e.target.value); }}
              >
                {RSS_INTERVAL_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>{o.label}</option>
                ))}
              </FormSelect>
              <p className="text-xs text-muted-foreground mt-1">{t("settings.prowlarrRssIntervalHelp")}</p>
            </FormField>
          </div>
          <div className="flex items-center gap-3">
            <Button
              onClick={handleTestConnection}
              disabled={isTesting || !prowlarrUrl || !prowlarrApiKey}
            >
              {isTesting ? (
                <>
                  <Icon name="spinner" size="sm" className="animate-spin -ml-1 mr-2" />
                  {t("settings.testing")}
                </>
              ) : (
                <>
                  <Icon name="refresh" size="sm" className="mr-2" />
                  {t("settings.testConnection")}
                </>
              )}
            </Button>
            {testResult && (
              <span className={`text-sm font-medium ${testResult.success ? "text-success" : "text-destructive"}`}>
                {testResult.message}
              </span>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

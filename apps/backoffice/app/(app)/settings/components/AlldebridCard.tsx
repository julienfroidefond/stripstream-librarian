"use client";

import { useEffect, useState } from "react";
import { SettingsCard, SettingsRow, FormInput, Icon, Switch } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { TestConnectionButton } from "./TestConnectionButton";

/**
 * AllDebrid settings card.
 *
 * The API key is write-only: the API never returns it (it is redacted), so the
 * input always starts empty and an empty submit keeps the stored key untouched.
 */
export function AlldebridCard({ handleUpdateSetting, initialData, onConfiguredChange }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void>; initialData: Record<string, unknown> | null; onConfiguredChange?: (configured: boolean) => void }) {
  const { t } = useTranslation();
  const [apiKey, setApiKey] = useState("");
  const [configured, setConfigured] = useState(
    typeof initialData?.api_key === "string" && initialData.api_key.trim().length > 0
  );
  const [enabled, setEnabled] = useState(initialData?.enabled === true);

  // Keep the parent (download-client picker) in sync: AllDebrid is only usable
  // when it is both enabled and holding a key.
  useEffect(() => {
    onConfiguredChange?.(configured && enabled);
  }, [configured, enabled, onConfiguredChange]);

  function save(overrides: Record<string, unknown> = {}) {
    const value: Record<string, unknown> = { enabled, ...overrides };
    const trimmed = apiKey.trim();
    if (trimmed) {
      value.api_key = trimmed;
      setConfigured(true);
      setApiKey("");
    }
    handleUpdateSetting("alldebrid", value);
  }

  return (
    <SettingsCard
      icon="globe"
      title={t("settings.alldebrid")}
      description={t("settings.alldebridDesc")}
      action={
        <Switch
          checked={enabled}
          aria-label={t("settings.alldebridEnabled")}
          onChange={(e) => {
            const val = e.target.checked;
            setEnabled(val);
            save({ enabled: val });
          }}
        />
      }
    >
      <SettingsRow label={t("settings.alldebridApiKey")}>
        <FormInput
          type="password"
          autoComplete="off"
          data-lpignore="true"
          data-1p-ignore
          data-bwignore
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onBlur={() => save()}
          placeholder={configured ? t("settings.alldebridApiKeyConfigured") : t("settings.alldebridApiKeyPlaceholder")}
        />
      </SettingsRow>

      <div className="pt-1">
        <TestConnectionButton
          endpoint="/api/alldebrid/test"
          disabled={!enabled || !configured}
        />
      </div>

      <div className="pt-1">
        <div className="flex items-start gap-2 rounded-xl bg-muted/40 p-3">
          <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t("settings.alldebridHelp")}</p>
        </div>
      </div>
    </SettingsCard>
  );
}

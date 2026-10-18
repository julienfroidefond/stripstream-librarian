"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, FormSelect, Icon, Tooltip } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { TestConnectionButton } from "./TestConnectionButton";

/** AllDebrid tuning read from `app_settings.limits`, with the server defaults. */
export interface AlldebridLimits {
  concurrent_alldebrid_downloads: number;
  alldebrid_stall_timeout_minutes: number;
}

/** Bounds mirror `apps/api/src/downloads/settings.rs`. */
export const ALLDEBRID_LIMIT_RANGES: Record<keyof AlldebridLimits, { min: number; max: number; default: number }> = {
  concurrent_alldebrid_downloads: { min: 1, max: 16, default: 4 },
  alldebrid_stall_timeout_minutes: { min: 1, max: 1440, default: 30 },
};

function clamp(key: keyof AlldebridLimits, raw: string): number {
  const { min, max, default: fallback } = ALLDEBRID_LIMIT_RANGES[key];
  const parsed = parseInt(raw, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

/**
 * AllDebrid settings card.
 *
 * The API key is write-only: the API never returns it (it is redacted), so the
 * input always starts empty and an empty submit keeps the stored key untouched.
 * The two tuning values live under `limits`, not under `alldebrid`, and are
 * saved through `onSaveLimits`.
 */
export function AlldebridCard({
  handleUpdateSetting,
  initialData,
  limits = {},
  onSaveLimits,
}: {
  handleUpdateSetting: (key: string, value: unknown) => Promise<void>;
  initialData: Record<string, unknown> | null;
  limits?: Partial<AlldebridLimits>;
  onSaveLimits?: (partial: Partial<AlldebridLimits>) => void;
}) {
  const { t } = useTranslation();
  const [apiKey, setApiKey] = useState("");
  const [configured, setConfigured] = useState(
    typeof initialData?.api_key === "string" && initialData.api_key.trim().length > 0
  );
  const [enabled, setEnabled] = useState(initialData?.enabled === true);
  // Raw strings so a value can be typed freely; clamped only on blur.
  const [concurrencyDraft, setConcurrencyDraft] = useState(
    String(
      typeof limits.concurrent_alldebrid_downloads === "number"
        ? limits.concurrent_alldebrid_downloads
        : ALLDEBRID_LIMIT_RANGES.concurrent_alldebrid_downloads.default
    )
  );
  const [stallTimeoutDraft, setStallTimeoutDraft] = useState(
    String(
      typeof limits.alldebrid_stall_timeout_minutes === "number"
        ? limits.alldebrid_stall_timeout_minutes
        : ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.default
    )
  );

  function commitLimit(key: keyof AlldebridLimits) {
    const draft = key === "concurrent_alldebrid_downloads" ? concurrencyDraft : stallTimeoutDraft;
    const value = clamp(key, draft);
    if (key === "concurrent_alldebrid_downloads") setConcurrencyDraft(String(value));
    else setStallTimeoutDraft(String(value));
    onSaveLimits?.({ [key]: value });
  }

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
    <SettingsCard icon="globe" title={t("settings.alldebrid")} description={t("settings.alldebridDesc")}>
      <SettingsRow label={t("settings.alldebridApiKey")}>
        <FormInput
          type="password"
          autoComplete="off"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onBlur={() => save()}
          placeholder={configured ? t("settings.alldebridApiKeyConfigured") : t("settings.alldebridApiKeyPlaceholder")}
        />
      </SettingsRow>

      <SettingsRow label={t("settings.alldebridEnabled")}>
        <FormSelect
          value={enabled ? "true" : "false"}
          onChange={(e) => {
            const val = e.target.value === "true";
            setEnabled(val);
            save({ enabled: val });
          }}
          className="w-40"
        >
          <option value="false">{t("common.disabled")}</option>
          <option value="true">{t("common.enabled")}</option>
        </FormSelect>
      </SettingsRow>

      <SettingsRow
        label={(
          <span className="flex items-center gap-1">
            {t("settings.alldebridConcurrency")}
            <Tooltip label={t("settings.alldebridConcurrencyHelp")}>
              <Icon name="info" size="sm" className="text-muted-foreground/60 cursor-default" />
            </Tooltip>
          </span>
        )}
        htmlFor="alldebrid-concurrency"
      >
        <FormInput
          id="alldebrid-concurrency"
          type="number"
          min={ALLDEBRID_LIMIT_RANGES.concurrent_alldebrid_downloads.min}
          max={ALLDEBRID_LIMIT_RANGES.concurrent_alldebrid_downloads.max}
          value={concurrencyDraft}
          onChange={(e) => setConcurrencyDraft(e.target.value)}
          onBlur={() => commitLimit("concurrent_alldebrid_downloads")}
          className="w-20"
        />
      </SettingsRow>

      <SettingsRow
        label={(
          <span className="flex items-center gap-1">
            {t("settings.alldebridStallTimeout")}
            <Tooltip label={t("settings.alldebridStallTimeoutHelp")}>
              <Icon name="info" size="sm" className="text-muted-foreground/60 cursor-default" />
            </Tooltip>
          </span>
        )}
        htmlFor="alldebrid-stall-timeout"
      >
        <FormInput
          id="alldebrid-stall-timeout"
          type="number"
          min={ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.min}
          max={ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.max}
          value={stallTimeoutDraft}
          onChange={(e) => setStallTimeoutDraft(e.target.value)}
          onBlur={() => commitLimit("alldebrid_stall_timeout_minutes")}
          className="w-24"
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

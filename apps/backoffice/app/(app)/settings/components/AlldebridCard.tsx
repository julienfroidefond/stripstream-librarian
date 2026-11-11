"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, Icon, Tooltip, Switch } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { AlldebridSettings, LimitsSettings } from "@/lib/settingsSections";
import { useSettingsDraft, useSettingsSection } from "../SettingsDraftProvider";
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
 * written to the shared limits slice of the draft.
 */
export function AlldebridCard() {
  const { t } = useTranslation();
  const draft = useSettingsDraft();
  const section = useSettingsSection<AlldebridSettings>("alldebrid");
  const value = section.value;
  const limits = useSettingsSection<LimitsSettings>("limits");
  const [apiKey, setApiKey] = useState("");
  // Raw strings so a value can be typed freely; clamped only on blur.
  const [concurrencyDraft, setConcurrencyDraft] = useState(
    String(limits.value.concurrent_alldebrid_downloads)
  );
  const [stallTimeoutDraft, setStallTimeoutDraft] = useState(
    String(limits.value.alldebrid_stall_timeout_minutes)
  );

  const configured = typeof value.api_key === "string" && value.api_key.trim().length > 0;

  async function save(overrides: Partial<AlldebridSettings> = {}) {
    const trimmed = apiKey.trim();
    const patch: Partial<AlldebridSettings> = { ...overrides };
    if (trimmed) patch.api_key = trimmed;
    section.patch(patch);
    if (!trimmed) {
      await section.save();
      return;
    }
    const result = await section.save();
    // Keep the typed key on screen when the write fails (validation or
    // upstream); clear it only once the server acknowledged the value. The
    // acknowledged section then carries the redacted placeholder.
    if (result.status === "saved") setApiKey("");
  }

  async function commitLimit(key: keyof AlldebridLimits) {
    const raw = key === "concurrent_alldebrid_downloads" ? concurrencyDraft : stallTimeoutDraft;
    const next = clamp(key, raw);
    if (key === "concurrent_alldebrid_downloads") {
      setConcurrencyDraft(String(next));
      limits.patch({ concurrent_alldebrid_downloads: next });
    } else {
      setStallTimeoutDraft(String(next));
      limits.patch({ alldebrid_stall_timeout_minutes: next });
    }
    const result = await limits.save();
    // Adopt a normalized or rolled-back value without touching the other row.
    if (result.status !== "unchanged") {
      const stored = draft.get<LimitsSettings>("limits");
      if (stored) {
        if (key === "concurrent_alldebrid_downloads") {
          setConcurrencyDraft(String(stored.concurrent_alldebrid_downloads));
        } else {
          setStallTimeoutDraft(String(stored.alldebrid_stall_timeout_minutes));
        }
      }
    }
  }

  return (
    <SettingsCard
      icon="globe"
      title={t("settings.alldebrid")}
      description={t("settings.alldebridDesc")}
      action={
        <Switch
          checked={value.enabled}
          aria-label={t("settings.alldebridEnabled")}
          onChange={(e) => void save({ enabled: e.target.checked })}
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
          onBlur={() => void save()}
          placeholder={configured ? t("settings.alldebridApiKeyConfigured") : t("settings.alldebridApiKeyPlaceholder")}
        />
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
          disabled={!value.enabled || !configured}
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

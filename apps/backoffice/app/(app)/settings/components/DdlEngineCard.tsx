"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, Icon, Tooltip } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n/fr";

/**
 * Download-engine knobs stored under `app_settings.limits` for the direct
 * (DDL) downloads. `download_max_attempts`, `download_stall_timeout_seconds`
 * and `download_timeout_minutes` are shared by every download engine, so the
 * labels spell that out; the two concurrency values are DDL-only.
 */
export interface DdlEngineLimits {
  concurrent_ddl_provider_downloads: number;
  ddl_download_connections: number;
  download_max_attempts: number;
  download_stall_timeout_seconds: number;
  download_timeout_minutes: number;
}

/** Bounds and defaults mirror `apps/api/src/downloads/settings.rs`. */
export const DDL_ENGINE_RANGES: Record<keyof DdlEngineLimits, { min: number; max: number; default: number }> = {
  concurrent_ddl_provider_downloads: { min: 1, max: 16, default: 2 },
  ddl_download_connections: { min: 1, max: 16, default: 4 },
  download_max_attempts: { min: 1, max: 20, default: 5 },
  download_stall_timeout_seconds: { min: 10, max: 3600, default: 180 },
  download_timeout_minutes: { min: 1, max: 1440, default: 30 },
};

const KEYS = Object.keys(DDL_ENGINE_RANGES) as (keyof DdlEngineLimits)[];

/** Parse an input field and clamp it into its range, falling back on empty. */
function clamp(key: keyof DdlEngineLimits, raw: string): number {
  const { min, max, default: fallback } = DDL_ENGINE_RANGES[key];
  const parsed = parseInt(raw, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

export function DdlEngineCard({
  limits,
  onSave,
}: {
  limits: Partial<DdlEngineLimits>;
  onSave: (partial: Partial<DdlEngineLimits>) => void;
}) {
  const { t } = useTranslation();
  // Raw strings so a value can be typed freely; clamped only on blur.
  const [drafts, setDrafts] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      KEYS.map((key) => {
        const stored = limits[key];
        const value = typeof stored === "number" && Number.isFinite(stored)
          ? stored
          : DDL_ENGINE_RANGES[key].default;
        return [key, String(value)];
      })
    )
  );

  function commit(key: keyof DdlEngineLimits) {
    const value = clamp(key, drafts[key]);
    setDrafts((prev) => ({ ...prev, [key]: String(value) }));
    onSave({ [key]: value });
  }

  function row(key: keyof DdlEngineLimits, labelKey: TranslationKey, helpKey: TranslationKey) {
    const { min, max } = DDL_ENGINE_RANGES[key];
    return (
      <SettingsRow
        label={(
          <span className="flex items-center gap-1">
            {t(labelKey)}
            <Tooltip label={t(helpKey)}>
              <Icon name="info" size="sm" className="text-muted-foreground/60 cursor-default" />
            </Tooltip>
          </span>
        )}
        htmlFor={`ddl-engine-${key}`}
      >
        <FormInput
          id={`ddl-engine-${key}`}
          type="number"
          min={min}
          max={max}
          value={drafts[key]}
          onChange={(e) => setDrafts((prev) => ({ ...prev, [key]: e.target.value }))}
          onBlur={() => commit(key)}
          className="w-20"
        />
      </SettingsRow>
    );
  }

  return (
    <SettingsCard icon="download" title={t("ddlEngine.title")} description={t("ddlEngine.desc")}>
      {row(
        "concurrent_ddl_provider_downloads",
        "ddlEngine.concurrent",
        "ddlEngine.concurrentHelp",
      )}
      {row("ddl_download_connections", "ddlEngine.connections", "ddlEngine.connectionsHelp")}
      {row("download_max_attempts", "ddlEngine.maxAttempts", "ddlEngine.maxAttemptsHelp")}
      {row(
        "download_stall_timeout_seconds",
        "ddlEngine.stallTimeout",
        "ddlEngine.stallTimeoutHelp",
      )}
      {row(
        "download_timeout_minutes",
        "ddlEngine.transferTimeout",
        "ddlEngine.transferTimeoutHelp",
      )}

      <div className="pt-1">
        <div className="flex items-start gap-2 rounded-xl bg-muted/40 p-3">
          <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t("ddlEngine.sharedNote")}</p>
        </div>
      </div>
    </SettingsCard>
  );
}

"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import {
  DEFAULT_AI_LISTS_PROMPT,
  DEFAULT_AI_PROMPT,
  type AiTaggingSettings,
} from "@/lib/settingsSections";
import { useSettingsSection } from "../SettingsDraftProvider";

export function AiTaggingCard() {
  const { t } = useTranslation();
  const section = useSettingsSection<AiTaggingSettings>("ai_tagging");
  const value = section.value;
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);

  const save = (patch: Partial<AiTaggingSettings>) => {
    section.patch(patch);
    void section.save();
  };

  const restoreDefaultPrompt = () => save({ prompt: DEFAULT_AI_PROMPT });
  const restoreDefaultListsPrompt = () => save({ reading_list_prompt: DEFAULT_AI_LISTS_PROMPT });

  const testConnection = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const response = await fetch("/api/settings/ai_tagging/test", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ base_url: value.base_url, api_key: value.api_key, model: value.model }),
      });
      const result = await response.json();
      setTestResult({ ok: response.ok, message: result.message ?? result.error ?? t("settings.aiTestError") });
    } catch {
      setTestResult({ ok: false, message: t("settings.aiTestError") });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2 xl:items-start">
        <SettingsCard icon="bot" title={t("settings.aiConfiguration")} description={t("settings.aiTaggingDesc")}>
          <label className="flex items-center gap-3 text-sm font-medium">
            <input type="checkbox" checked={value.enabled} onChange={e => save({ enabled: e.target.checked })} />
            {t("settings.aiGenreTagging")}
          </label>
          <label className="flex items-center gap-3 text-sm font-medium">
            <input type="checkbox" checked={value.reading_lists_enabled} onChange={e => save({ reading_lists_enabled: e.target.checked })} />
            {t("settings.aiReadingListsEnabled")}
          </label>
          <SettingsRow label={t("settings.aiBaseUrl")} htmlFor="ai-base-url">
            <FormInput id="ai-base-url" value={value.base_url} onChange={e => section.patch({ base_url: e.target.value })} onBlur={() => void section.save()} placeholder="https://openrouter.ai/api/v1" />
          </SettingsRow>
          <SettingsRow label={t("settings.aiApiKey")} htmlFor="ai-api-key">
            <FormInput id="ai-api-key" type="password" value={value.api_key} onChange={e => section.patch({ api_key: e.target.value })} onBlur={() => void section.save()} placeholder="sk-or-v1-..." autoComplete="off" />
          </SettingsRow>
          <SettingsRow label={t("settings.aiModel")} htmlFor="ai-model">
            <FormInput id="ai-model" value={value.model} onChange={e => section.patch({ model: e.target.value })} onBlur={() => void section.save()} placeholder="openrouter/free" />
          </SettingsRow>
          <SettingsRow label={t("settings.aiMaxTags")} htmlFor="ai-max-tags">
            <FormInput id="ai-max-tags" type="number" min={1} max={10} value={value.max_tags} onChange={e => section.patch({ max_tags: Math.min(10, Math.max(1, parseInt(e.target.value) || 5)) })} onBlur={() => void section.save()} className="w-24" />
          </SettingsRow>
          <div className="flex flex-wrap items-center gap-3 pt-2">
            <button type="button" onClick={testConnection} disabled={testing} className="rounded-xl bg-primary px-3.5 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50">
              {testing ? t("settings.aiTesting") : t("settings.aiTest")}
            </button>
            {testResult && <p className={`text-sm ${testResult.ok ? "text-success" : "text-destructive"}`}>{testResult.message}</p>}
          </div>
        </SettingsCard>

        <SettingsCard icon="document" title={t("settings.aiPrompt")} description={t("settings.aiPromptHelp")}>
          <textarea
            id="ai-prompt"
            value={value.prompt}
            onChange={e => section.patch({ prompt: e.target.value })}
            onBlur={() => void section.save()}
            rows={14}
            className="w-full px-3.5 py-3 font-mono text-sm leading-6 text-foreground"
          />
          <div className="flex justify-end">
            <button
              type="button"
              onClick={restoreDefaultPrompt}
              disabled={value.prompt === DEFAULT_AI_PROMPT}
              className="text-xs font-medium text-primary hover:underline disabled:cursor-not-allowed disabled:opacity-50"
            >
              {t("settings.aiRestoreDefaultPrompt")}
            </button>
          </div>
        </SettingsCard>
      </div>

      <SettingsCard icon="document" title={t("settings.aiReadingListsTitle")} description={t("settings.aiReadingListsHelp")}>
        <textarea
          id="ai-reading-lists-prompt"
          value={value.reading_list_prompt}
          onChange={e => section.patch({ reading_list_prompt: e.target.value })}
          onBlur={() => void section.save()}
          aria-label={t("settings.aiReadingListsTitle")}
          rows={12}
          className="w-full px-3.5 py-3 font-mono text-sm leading-6 text-foreground"
        />
        <div className="flex justify-end">
          <button
            type="button"
            onClick={restoreDefaultListsPrompt}
            disabled={value.reading_list_prompt === DEFAULT_AI_LISTS_PROMPT}
            className="text-xs font-medium text-primary hover:underline disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t("settings.aiRestoreDefaultListsPrompt")}
          </button>
        </div>
      </SettingsCard>
    </div>
  );
}

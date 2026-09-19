"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormInput, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

const DEFAULT_PROMPT = "Suggest up to {{max_tags}} relevant genres for each comic or manga series. Usually return only 1 to 3 genres; do not fill the limit when fewer genres fit.\n\nYou MUST choose tags only from this existing genre list, preserving the exact spelling and casing: {{genres}}. Never create or paraphrase a genre.\n\nUse the supplied metadata to disambiguate the series. For a recognizable, well-known title, you may use reliable general knowledge even when its metadata is sparse. Do not make uncertain associations: omit a series if you cannot identify it with confidence. Do not select a format, medium, age category, or generic label merely because the series is a comic (for example 'BD' or 'Books/Comics'). Select those labels only if they are genuinely relevant genres for the series. Return only JSON in the form {\"suggestions\":[{\"series_id\":\"uuid\",\"tags\":[\"existing genre\"]}]}.\n\nSeries: {{series}}";
type AiTagging = { enabled?: boolean; base_url?: string; api_key?: string; model?: string; max_tags?: number; prompt?: string };

export function AiTaggingCard({ initialData, handleUpdateSetting }: { initialData: AiTagging | null; handleUpdateSetting: (key: string, value: unknown) => Promise<void> }) {
  const { t } = useTranslation();
  const [data, setData] = useState<Required<AiTagging>>({
    enabled: initialData?.enabled ?? false,
    base_url: initialData?.base_url ?? "https://openrouter.ai/api/v1",
    api_key: initialData?.api_key ?? "",
    model: initialData?.model ?? "openrouter/free",
    max_tags: initialData?.max_tags ?? 5,
    prompt: initialData?.prompt ?? DEFAULT_PROMPT,
  });
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);

  const save = (next: Required<AiTagging>) => {
    setData(next);
    void handleUpdateSetting("ai_tagging", next);
  };

  const restoreDefaultPrompt = () => save({ ...data, prompt: DEFAULT_PROMPT });

  const testConnection = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const response = await fetch("/api/settings/ai_tagging/test", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ base_url: data.base_url, api_key: data.api_key, model: data.model }),
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
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2"><Icon name="bot" size="md" />{t("settings.aiConfiguration")}</CardTitle>
        <CardDescription>{t("settings.aiTaggingDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          <label className="flex items-center gap-3 text-sm font-medium">
            <input type="checkbox" checked={data.enabled} onChange={e => save({ ...data, enabled: e.target.checked })} />
            {t("settings.aiGenreTagging")}
          </label>
          <FormField>
            <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.aiBaseUrl")}</label>
            <FormInput value={data.base_url} onChange={e => setData({ ...data, base_url: e.target.value })} onBlur={() => save(data)} placeholder="https://openrouter.ai/api/v1" />
          </FormField>
          <FormField>
            <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.aiApiKey")}</label>
            <FormInput type="password" value={data.api_key} onChange={e => setData({ ...data, api_key: e.target.value })} onBlur={() => save(data)} placeholder="sk-or-v1-..." autoComplete="off" />
          </FormField>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <FormField>
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.aiModel")}</label>
              <FormInput value={data.model} onChange={e => setData({ ...data, model: e.target.value })} onBlur={() => save(data)} placeholder="openrouter/free" />
            </FormField>
            <FormField>
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.aiMaxTags")}</label>
              <FormInput type="number" min={1} max={10} value={data.max_tags} onChange={e => setData({ ...data, max_tags: Math.min(10, Math.max(1, parseInt(e.target.value) || 5)) })} onBlur={() => save(data)} />
            </FormField>
          </div>
          <FormField>
            <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.aiPrompt")}</label>
            <textarea
              value={data.prompt}
              onChange={e => setData({ ...data, prompt: e.target.value })}
              onBlur={() => save(data)}
              rows={9}
              className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm leading-6 text-foreground focus:outline-none focus:ring-2 focus:ring-primary font-mono"
            />
            <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
              <p className="text-xs text-muted-foreground">{t("settings.aiPromptHelp")}</p>
              <button
                type="button"
                onClick={restoreDefaultPrompt}
                disabled={data.prompt === DEFAULT_PROMPT}
                className="text-xs font-medium text-primary hover:underline disabled:cursor-not-allowed disabled:opacity-50"
              >
                {t("settings.aiRestoreDefaultPrompt")}
              </button>
            </div>
          </FormField>
          <div className="flex flex-wrap items-center gap-3 pt-2">
            <button type="button" onClick={testConnection} disabled={testing} className="rounded-lg border border-primary/40 px-3 py-2 text-sm font-medium text-primary hover:bg-primary/10 disabled:opacity-50">
              {testing ? t("settings.aiTesting") : t("settings.aiTest")}
            </button>
            {testResult && <p className={`text-sm ${testResult.ok ? "text-success" : "text-destructive"}`}>{testResult.message}</p>}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

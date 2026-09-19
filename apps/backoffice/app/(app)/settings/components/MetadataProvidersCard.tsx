"use client";

import { useEffect, useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormInput, FormSelect, Icon } from "@/app/components/ui";
import { ProviderIcon } from "@/app/components/ProviderIcon";
import type { MetadataProviderDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

export const METADATA_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
] as const;

function extractInitialApiKeys(data: Record<string, unknown> | null): Record<string, string> {
  const keys: Record<string, string> = {};
  if (data) {
    const comicvine = data.comicvine as Record<string, unknown> | undefined;
    const googleBooks = data.google_books as Record<string, unknown> | undefined;
    if (comicvine?.api_key) keys.comicvine = String(comicvine.api_key);
    if (googleBooks?.api_key) keys.google_books = String(googleBooks.api_key);
  }
  return keys;
}

export function MetadataProvidersCard({ handleUpdateSetting, initialData }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void>; initialData: Record<string, unknown> | null }) {
  const { t } = useTranslation();
  const [defaultProvider, setDefaultProvider] = useState(initialData?.default_provider ? String(initialData.default_provider) : "google_books");
  const [metadataLanguage, setMetadataLanguage] = useState(initialData?.metadata_language ? String(initialData.metadata_language) : "en");
  const [apiKeys, setApiKeys] = useState<Record<string, string>>(extractInitialApiKeys(initialData));
  const [providers, setProviders] = useState<MetadataProviderDto[]>([]);

  useEffect(() => {
    fetch("/api/metadata/providers")
      .then((response) => response.ok ? response.json() : Promise.reject(new Error("provider list failed")))
      .then((data: MetadataProviderDto[]) => setProviders(data))
      .catch(() => setProviders([]));
  }, []);

  function save(provider: string, lang: string, keys: Record<string, string>) {
    const value: Record<string, unknown> = {
      default_provider: provider,
      metadata_language: lang,
    };
    for (const [k, v] of Object.entries(keys)) {
      if (v) value[k] = { api_key: v };
    }
    handleUpdateSetting("metadata_providers", value);
  }

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="search" size="md" />
          {t("settings.metadataProviders")}
        </CardTitle>
        <CardDescription>{t("settings.metadataProvidersDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-6">
          {/* Default provider */}
          <div>
            <label className="text-sm font-medium text-muted-foreground mb-2 block">{t("settings.defaultProvider")}</label>
            <div className="flex gap-2 flex-wrap">
              {providers.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => {
                    setDefaultProvider(p.id);
                    save(p.id, metadataLanguage, apiKeys);
                  }}
                  className={`inline-flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium border transition-colors ${
                    defaultProvider === p.id
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary/50"
                  }`}
                >
                  <ProviderIcon provider={p.id} size={18} />
                  {p.label}
                </button>
              ))}
            </div>
            <p className="text-xs text-muted-foreground mt-2">{t("settings.defaultProviderHelp")}</p>
          </div>

          {/* Metadata language */}
          <div>
            <label className="text-sm font-medium text-muted-foreground mb-2 block">{t("settings.metadataLanguage")}</label>
            <div className="flex gap-2">
              {METADATA_LANGUAGES.map((l) => (
                <button
                  key={l.value}
                  type="button"
                  onClick={() => {
                    setMetadataLanguage(l.value);
                    save(defaultProvider, l.value, apiKeys);
                  }}
                  className={`px-3 py-2 rounded-lg text-sm font-medium border transition-colors ${
                    metadataLanguage === l.value
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary/50"
                  }`}
                >
                  {l.label}
                </button>
              ))}
            </div>
            <p className="text-xs text-muted-foreground mt-2">{t("settings.metadataLanguageHelp")}</p>
          </div>

          {/* Provider API keys — always visible */}
          <div className="border-t border-border/50 pt-4">
            <h4 className="text-sm font-medium text-foreground mb-3">{t("settings.apiKeys")}</h4>
            <div className="space-y-4">
              <FormField>
                <label className="text-sm font-medium text-muted-foreground mb-1 flex items-center gap-1.5">
                  <ProviderIcon provider="google_books" size={16} />
                  {t("settings.googleBooksKey")}
                </label>
                <FormInput
                  type="password" autoComplete="off"
                  placeholder={t("settings.googleBooksPlaceholder")}
                  value={apiKeys.google_books || ""}
                  onChange={(e) => setApiKeys({ ...apiKeys, google_books: e.target.value })}
                  onBlur={() => save(defaultProvider, metadataLanguage, apiKeys)}
                />
                <p className="text-xs text-muted-foreground mt-1">{t("settings.googleBooksHelp")}</p>
              </FormField>

              <FormField>
                <label className="text-sm font-medium text-muted-foreground mb-1 flex items-center gap-1.5">
                  <ProviderIcon provider="comicvine" size={16} />
                  {t("settings.comicvineKey")}
                </label>
                <FormInput
                  type="password" autoComplete="off"
                  placeholder={t("settings.comicvinePlaceholder")}
                  value={apiKeys.comicvine || ""}
                  onChange={(e) => setApiKeys({ ...apiKeys, comicvine: e.target.value })}
                  onBlur={() => save(defaultProvider, metadataLanguage, apiKeys)}
                />
                <p className="text-xs text-muted-foreground mt-1">{t("settings.comicvineHelp")} <span className="font-mono text-foreground/70">comicvine.gamespot.com/api</span>.</p>
              </FormField>

              <div className="p-3 rounded-lg bg-muted/30 flex items-center gap-3 flex-wrap">
                {providers.filter((provider) => !provider.requires_api_key).map((provider, index) => (
                  <span key={provider.id} className="contents">
                    {index > 0 && <span className="text-xs text-muted-foreground">{t("common.and")}</span>}
                    <span className="flex items-center gap-1.5">
                      <ProviderIcon provider={provider.id} size={16} />
                      <span className="text-xs font-medium text-foreground">{provider.label}</span>
                    </span>
                  </span>
                ))}
                <span className="text-xs text-muted-foreground">{t("settings.freeProviders")}</span>
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

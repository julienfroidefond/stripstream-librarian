"use client";

import { useState, useEffect } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormInput, FormSelect, Icon } from "@/app/components/ui";
import { ProviderIcon } from "@/app/components/ProviderIcon";
import { useTranslation } from "@/lib/i18n/context";

export const METADATA_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
] as const;

export function MetadataProvidersCard({ handleUpdateSetting }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void> }) {
  const { t } = useTranslation();
  const [defaultProvider, setDefaultProvider] = useState("google_books");
  const [metadataLanguage, setMetadataLanguage] = useState("en");
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});

  useEffect(() => {
    fetch("/api/settings/metadata_providers")
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data) {
          if (data.default_provider) setDefaultProvider(data.default_provider);
          if (data.metadata_language) setMetadataLanguage(data.metadata_language);
          if (data.comicvine?.api_key) setApiKeys((prev) => ({ ...prev, comicvine: data.comicvine.api_key }));
          if (data.google_books?.api_key) setApiKeys((prev) => ({ ...prev, google_books: data.google_books.api_key }));
        }
      })
      .catch(() => {});
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
              {([
                { value: "google_books", label: "Google Books" },
                { value: "open_library", label: "Open Library" },
                { value: "comicvine", label: "ComicVine" },
                { value: "anilist", label: "AniList" },
                { value: "bedetheque", label: "Bédéthèque" },
              ] as const).map((p) => (
                <button
                  key={p.value}
                  type="button"
                  onClick={() => {
                    setDefaultProvider(p.value);
                    save(p.value, metadataLanguage, apiKeys);
                  }}
                  className={`inline-flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium border transition-colors ${
                    defaultProvider === p.value
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary/50"
                  }`}
                >
                  <ProviderIcon provider={p.value} size={18} />
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
                <div className="flex items-center gap-1.5">
                  <ProviderIcon provider="open_library" size={16} />
                  <span className="text-xs font-medium text-foreground">Open Library</span>
                </div>
                <span className="text-xs text-muted-foreground">,</span>
                <div className="flex items-center gap-1.5">
                  <ProviderIcon provider="anilist" size={16} />
                  <span className="text-xs font-medium text-foreground">AniList</span>
                </div>
                <span className="text-xs text-muted-foreground">{t("common.and")}</span>
                <div className="flex items-center gap-1.5">
                  <ProviderIcon provider="bedetheque" size={16} />
                  <span className="text-xs font-medium text-foreground">Bédéthèque</span>
                </div>
                <span className="text-xs text-muted-foreground">{t("settings.freeProviders")}</span>
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

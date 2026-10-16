"use client";

import { useEffect, useState } from "react";
import { SettingsCard, SettingsRow, FormInput } from "@/app/components/ui";
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
    <SettingsCard icon="search" title={t("settings.metadataProviders")} description={t("settings.metadataProvidersDesc")}>
      {/* Default provider */}
      <SettingsRow label={t("settings.defaultProvider")} description={t("settings.defaultProviderHelp")} controlClassName="sm:max-w-xl">
        <div className="flex flex-wrap gap-2">
          {providers.map((p) => (
            <button
              key={p.id}
              type="button"
              onClick={() => {
                setDefaultProvider(p.id);
                save(p.id, metadataLanguage, apiKeys);
              }}
              className={`inline-flex items-center gap-2 rounded-full px-3.5 py-1.5 text-sm transition-colors ${
                defaultProvider === p.id
                  ? "bg-primary/15 font-semibold text-primary"
                  : "settings-well text-muted-foreground hover:text-foreground"
              }`}
            >
              <ProviderIcon provider={p.id} size={18} />
              {p.label}
            </button>
          ))}
        </div>
      </SettingsRow>

      {/* Metadata language */}
      <SettingsRow label={t("settings.metadataLanguage")} description={t("settings.metadataLanguageHelp")} controlClassName="sm:max-w-xl">
        <div className="flex gap-2">
          {METADATA_LANGUAGES.map((l) => (
            <button
              key={l.value}
              type="button"
              onClick={() => {
                setMetadataLanguage(l.value);
                save(defaultProvider, l.value, apiKeys);
              }}
              className={`rounded-full px-3.5 py-1.5 text-sm transition-colors ${
                metadataLanguage === l.value
                  ? "bg-primary/15 font-semibold text-primary"
                  : "settings-well text-muted-foreground hover:text-foreground"
              }`}
            >
              {l.label}
            </button>
          ))}
        </div>
      </SettingsRow>

      {/* Provider API keys — always visible */}
      <div className="space-y-4 pt-2">
        <h4 className="text-sm font-semibold text-foreground">{t("settings.apiKeys")}</h4>
        <SettingsRow
          label={<span className="flex items-center gap-1.5"><ProviderIcon provider="google_books" size={16} />{t("settings.googleBooksKey")}</span>}
          description={t("settings.googleBooksHelp")}
          htmlFor="metadata-google-books-key"
        >
          <FormInput
            id="metadata-google-books-key"
            type="password" autoComplete="off"
            placeholder={t("settings.googleBooksPlaceholder")}
            value={apiKeys.google_books || ""}
            onChange={(e) => setApiKeys({ ...apiKeys, google_books: e.target.value })}
            onBlur={() => save(defaultProvider, metadataLanguage, apiKeys)}
          />
        </SettingsRow>

        <SettingsRow
          label={<span className="flex items-center gap-1.5"><ProviderIcon provider="comicvine" size={16} />{t("settings.comicvineKey")}</span>}
          description={<>{t("settings.comicvineHelp")} <span className="font-mono text-foreground/70">comicvine.gamespot.com/api</span>.</>}
          htmlFor="metadata-comicvine-key"
        >
          <FormInput
            id="metadata-comicvine-key"
            type="password" autoComplete="off"
            placeholder={t("settings.comicvinePlaceholder")}
            value={apiKeys.comicvine || ""}
            onChange={(e) => setApiKeys({ ...apiKeys, comicvine: e.target.value })}
            onBlur={() => save(defaultProvider, metadataLanguage, apiKeys)}
          />
        </SettingsRow>

        <div className="settings-well flex flex-wrap items-center gap-3 rounded-xl px-4 py-3">
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
    </SettingsCard>
  );
}

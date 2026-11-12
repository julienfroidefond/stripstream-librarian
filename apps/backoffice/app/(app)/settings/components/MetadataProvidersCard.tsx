"use client";

import { useEffect, useState } from "react";
import { SettingsCard, SettingsRow, FormInput } from "@/app/components/ui";
import { ProviderIcon } from "@/app/components/ProviderIcon";
import type { MetadataProviderDto } from "@/lib/api";
import { browserApi } from "@/lib/api/browser";
import { useTranslation } from "@/lib/i18n/context";
import type { MetadataProvidersSettings } from "@/lib/settingsSections";
import { useSettingsDraft, useSettingsSection } from "../SettingsDraftProvider";

export const METADATA_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
] as const;

function extractApiKeys(data: MetadataProvidersSettings): Record<string, string> {
  const keys: Record<string, string> = {};
  if (data.comicvine?.api_key) keys.comicvine = data.comicvine.api_key;
  if (data.google_books?.api_key) keys.google_books = data.google_books.api_key;
  return keys;
}

export function MetadataProvidersCard() {
  const { t } = useTranslation();
  const draft = useSettingsDraft();
  const section = useSettingsSection<MetadataProvidersSettings>("metadata_providers");
  const value = section.value;
  const defaultProvider = value.default_provider;
  const metadataLanguage = value.metadata_language;
  // Raw key inputs so they can be typed; re-synced from the acknowledged
  // section when a save normalizes or a rollback restores it.
  const [apiKeys, setApiKeys] = useState<Record<string, string>>(() => extractApiKeys(value));
  const [providers, setProviders] = useState<MetadataProviderDto[]>([]);

  useEffect(() => {
    browserApi.metadata.providers()
      .then((data) => setProviders(data))
      .catch(() => setProviders([]));
  }, []);

  async function save(provider: string, lang: string, keys: Record<string, string>) {
    const next: MetadataProvidersSettings = {
      default_provider: provider,
      metadata_language: lang,
    };
    if (keys.google_books) next.google_books = { api_key: keys.google_books };
    if (keys.comicvine) next.comicvine = { api_key: keys.comicvine };
    section.set(next);
    const result = await section.save();
    if (result.status !== "unchanged") {
      const stored = draft.get<MetadataProvidersSettings>("metadata_providers");
      if (stored) setApiKeys(extractApiKeys(stored));
    }
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
              onClick={() => save(p.id, metadataLanguage, apiKeys)}
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
              onClick={() => save(defaultProvider, l.value, apiKeys)}
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

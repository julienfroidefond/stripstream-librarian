"use client";

import { useEffect, useState, type ReactNode } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Button, FormInput, FormSelect, Icon, SettingsCard, SettingsPageHeader, SettingsRow, SettingsSections, SettingsTabs, Switch, toast, type SettingsTabItem } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import { Settings, CacheStats, ClearCacheResponse, ThumbnailStats, UserDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import type { Locale } from "@/lib/i18n/types";
import { MetadataProvidersCard } from "./components/MetadataProvidersCard";
import { StatusMappingsCard } from "./components/StatusMappingsCard";
import { ProwlarrCard } from "./components/ProwlarrCard";
import { QBittorrentCard } from "./components/QBittorrentCard";
import { AlldebridCard } from "./components/AlldebridCard";
import { TelegramCard } from "./components/TelegramCard";
import { KomgaSyncCard } from "./components/KomgaSyncCard";
import { AnilistTab } from "./components/AnilistTab";
import { RenameFormatCard } from "./components/RenameFormatCard";
import { ArchivesTab } from "./components/ArchivesTab";
import { TelegramMonitorCard } from "./components/TelegramMonitorCard";
import { DdlProvidersCard } from "./components/DdlProvidersCard";
import { AiTaggingCard } from "./components/AiTaggingCard";

interface SettingsPageProps {
  initialSettings: Settings;
  initialCacheStats: CacheStats;
  initialThumbnailStats: ThumbnailStats;
  users: UserDto[];
  initialTab?: string;
  initialProwlarr: Record<string, unknown> | null;
  initialQbittorrent: Record<string, unknown> | null;
  initialAlldebrid: Record<string, unknown> | null;
  initialTorrentImport: Record<string, unknown> | null;
  initialTelegram: Record<string, unknown> | null;
  initialAnilist: Record<string, unknown> | null;
  initialKomga: Record<string, unknown> | null;
  initialMetadataProviders: Record<string, unknown> | null;
  initialStatusMappings: Record<string, unknown>[];
  initialSeriesStatuses: string[];
  initialProviderStatuses: string[];
  initialRenameFormat: string | null;
  initialRenameFormatHs: string | null;
  initialRenameFormatInt?: string | null;
  initialRenameFormatOneshot?: string | null;
  initialDownloadsEnabled: boolean;
  initialLibraries: import("@/lib/api").LibraryDto[];
  versions?: { api: string; indexer: string; backoffice: string };
  tokensContent?: ReactNode;
}

export default function SettingsPage({ initialSettings, initialCacheStats, initialThumbnailStats, users, initialTab, initialProwlarr, initialQbittorrent, initialAlldebrid, initialTorrentImport, initialTelegram, initialAnilist, initialKomga, initialMetadataProviders, initialStatusMappings, initialSeriesStatuses, initialProviderStatuses, initialRenameFormat, initialRenameFormatHs, initialRenameFormatInt, initialRenameFormatOneshot, initialDownloadsEnabled, initialLibraries, versions, tokensContent }: SettingsPageProps) {
  const { t, locale, setLocale } = useTranslation();
  const router = useRouter();
  const searchParams = useSearchParams();

  const [settings, setSettings] = useState<Settings>({
    ...initialSettings,
    thumbnail: initialSettings.thumbnail || { enabled: true, width: 300, height: 400, quality: 80, format: "webp", directory: "/data/thumbnails" }
  });
  const [cacheStats, setCacheStats] = useState<CacheStats>(initialCacheStats);
  const [thumbnailStats, setThumbnailStats] = useState<ThumbnailStats>(initialThumbnailStats);
  const [isClearing, setIsClearing] = useState(false);
  const [clearResult, setClearResult] = useState<ClearCacheResponse | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [downloadsEnabled, setDownloadsEnabled] = useState(initialDownloadsEnabled);

  const VALID_TABS = ["general", "media", "downloadTools", "metadata", "ai", "readingStatus", "notifications", "tokens", "archives"] as const;
  type TabId = typeof VALID_TABS[number];

  function resolveTab(tab: string | null | undefined): TabId {
    if (tab && (VALID_TABS as readonly string[]).includes(tab)) return tab as TabId;
    return "general";
  }

  const [activeTab, setActiveTab] = useState<TabId>(
    resolveTab(searchParams.get("tab") ?? initialTab)
  );

  useEffect(() => {
    if (activeTab !== "media") return;

    let active = true;
    const refreshCacheStats = async () => {
      const response = await fetch("/api/settings/cache/stats", { cache: "no-store" }).catch(() => null);
      if (active && response?.ok) setCacheStats(await response.json());
    };
    void refreshCacheStats();
    const interval = window.setInterval(refreshCacheStats, 5000);
    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, [activeTab]);

  function handleTabChange(tab: TabId) {
    setActiveTab(tab);
    router.replace(`?tab=${tab}`, { scroll: false });
  }

  function hasEmptyValue(v: unknown): boolean {
    if (v === null || v === "") return true;
    if (typeof v === "object" && v !== null) {
      return Object.values(v).some((val) => val !== undefined && hasEmptyValue(val));
    }
    return false;
  }

  async function handleUpdateSetting(key: string, value: unknown) {
    if (hasEmptyValue(value)) return;
    setIsSaving(true);
    try {
      const response = await fetch(`/api/settings/${key}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ value })
      });
      if (response.ok) {
        toast(t("settings.savedSuccess"), "success");
      } else {
        toast(t("settings.savedError"), "error");
      }
    } catch {
      toast(t("settings.saveError"), "error");
    } finally {
      setIsSaving(false);
    }
  }

  async function handleClearCache() {
    setIsClearing(true);
    setClearResult(null);
    try {
      const response = await fetch("/api/settings/cache/clear", { method: "POST" });
      const result = await response.json();
      setClearResult(result);
      // Refresh cache stats
      const statsResponse = await fetch("/api/settings/cache/stats");
      if (statsResponse.ok) {
        const stats = await statsResponse.json();
        setCacheStats(stats);
      }
    } catch {
      setClearResult({ success: false, message: t("settings.cacheClearError") });
    } finally {
      setIsClearing(false);
    }
  }

  const tabs: SettingsTabItem[] = [
    { id: "general", label: t("settings.general"), icon: "settings" },
    { id: "media", label: t("settings.media"), icon: "image" },
    { id: "downloadTools", label: t("settings.downloadTools"), icon: "play" },
    { id: "metadata", label: t("settings.metadata"), icon: "tag" },
    { id: "ai", label: t("settings.aiTab"), icon: "bot" },
    { id: "readingStatus", label: t("settings.readingStatus"), icon: "eye" },
    { id: "notifications", label: t("settings.notifications"), icon: "bell" },
    { id: "tokens", label: t("nav.tokens"), icon: "tokens" },
    { id: "archives", label: t("settings.archives"), icon: "books" },
  ];

  return (
    <div className="settings-surface">
      <SettingsPageHeader icon="settings" title={t("settings.title")} description={t("settings.subtitle")} />

      <SettingsTabs
        tabs={tabs}
        active={activeTab}
        onChange={(id) => handleTabChange(id as TabId)}
        trailing={
          isSaving ? (
            <span className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
              <Icon name="spinner" size="sm" mono className="animate-spin" />
              {t("settings.saving")}
            </span>
          ) : undefined
        }
      />

      {activeTab === "general" && (<SettingsSections>
      {/* Language Selector */}
      <SettingsCard icon="globe" title={t("settings.language")} description={t("settings.languageDesc")}>
        <div className="max-w-xs">
          <FormSelect
            value={locale}
            onChange={(e) => setLocale(e.target.value as Locale)}
          >
            <option value="fr">Français</option>
            <option value="en">English</option>
          </FormSelect>
        </div>
      </SettingsCard>

      {/* Limits Settings */}
      <SettingsCard icon="performance" title={t("settings.performanceLimits")} description={t("settings.performanceDesc")}>
        <SettingsRow label={t("settings.concurrentRenders")} description={t("settings.concurrentRendersHelp")}>
          <FormInput
            type="number"
            min={1}
            max={20}
            value={settings.limits.concurrent_renders}
            onChange={(e) => {
              const concurrent_renders = parseInt(e.target.value) || 4;
              const newSettings = { ...settings, limits: { ...settings.limits, concurrent_renders } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("limits", settings.limits)}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.timeoutSeconds")}>
          <FormInput
            type="number"
            min={5}
            max={300}
            value={settings.limits.timeout_seconds}
            onChange={(e) => {
              const timeout_seconds = parseInt(e.target.value) || 12;
              const newSettings = { ...settings, limits: { ...settings.limits, timeout_seconds } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("limits", settings.limits)}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.rateLimit")}>
          <FormInput
            type="number"
            min={10}
            max={1000}
            value={settings.limits.rate_limit_per_second}
            onChange={(e) => {
              const rate_limit_per_second = parseInt(e.target.value) || 120;
              const newSettings = { ...settings, limits: { ...settings.limits, rate_limit_per_second } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("limits", settings.limits)}
            className="w-32"
          />
        </SettingsRow>
        <p className="settings-well flex items-start gap-2.5 rounded-xl p-3.5 text-sm leading-relaxed text-muted-foreground">
          <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-primary" />
          {t("settings.limitsNote")}
        </p>
      </SettingsCard>

      {/* Rename Format */}
      <RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat={initialRenameFormat} initialRenameFormatHs={initialRenameFormatHs} initialRenameFormatInt={initialRenameFormatInt} initialRenameFormatOneshot={initialRenameFormatOneshot} />

      {/* About */}
      <SettingsCard icon="document" title={t("settings.about")}>
        <div>
          <p className="text-sm font-medium text-foreground">Stripstream Librarian</p>
          <p className="mt-0.5 text-sm leading-relaxed text-muted-foreground">{t("settings.aboutDesc")}</p>
        </div>

        <dl className="settings-well grid grid-cols-1 gap-x-6 gap-y-4 rounded-xl p-4 sm:grid-cols-3">
          <div>
            <dt className="text-xs text-muted-foreground">API</dt>
            <dd className="mt-0.5 text-sm font-medium tabular-nums text-foreground">{versions?.api ?? "?"}</dd>
          </div>
          <div>
            <dt className="text-xs text-muted-foreground">Indexer</dt>
            <dd className="mt-0.5 text-sm font-medium tabular-nums text-foreground">{versions?.indexer ?? "?"}</dd>
          </div>
          <div>
            <dt className="text-xs text-muted-foreground">Backoffice</dt>
            <dd className="mt-0.5 text-sm font-medium tabular-nums text-foreground">{versions?.backoffice ?? "?"}</dd>
          </div>
        </dl>

        <a
          href="https://github.com/julienfroidefond/stripstream-librarian"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 text-sm font-medium text-primary hover:underline"
        >
          <Icon name="externalLink" size="sm" mono />
          GitHub
        </a>
      </SettingsCard>

      </SettingsSections>)}

      {activeTab === "media" && (<SettingsSections>
      {/* Image Processing Settings */}
      <SettingsCard icon="image" title={t("settings.imageProcessing")} description={<SafeHtml html={t("settings.imageProcessingDesc")} as="span" />}>
        <SettingsRow label={t("settings.defaultFormat")}>
          <FormSelect
            value={settings.image_processing.format}
            onChange={(e) => {
              const newSettings = { ...settings, image_processing: { ...settings.image_processing, format: e.target.value } };
              setSettings(newSettings);
              handleUpdateSetting("image_processing", newSettings.image_processing);
            }}
          >
            <option value="webp">WebP</option>
            <option value="jpeg">JPEG</option>
            <option value="png">PNG</option>
          </FormSelect>
        </SettingsRow>
        <SettingsRow label={t("settings.defaultQuality")}>
          <FormInput
            type="number"
            min={1}
            max={100}
            value={settings.image_processing.quality}
            onChange={(e) => {
              const quality = parseInt(e.target.value) || 85;
              const newSettings = { ...settings, image_processing: { ...settings.image_processing, quality } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("image_processing", settings.image_processing)}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.defaultFilter")}>
          <FormSelect
            value={settings.image_processing.filter}
            onChange={(e) => {
              const newSettings = { ...settings, image_processing: { ...settings.image_processing, filter: e.target.value } };
              setSettings(newSettings);
              handleUpdateSetting("image_processing", newSettings.image_processing);
            }}
          >
            <option value="lanczos3">{t("settings.filterLanczos")}</option>
            <option value="triangle">{t("settings.filterTriangle")}</option>
            <option value="nearest">{t("settings.filterNearest")}</option>
          </FormSelect>
        </SettingsRow>
        <SettingsRow label={t("settings.maxWidth")}>
          <FormInput
            type="number"
            min={100}
            max={2160}
            value={settings.image_processing.max_width}
            onChange={(e) => {
              const max_width = parseInt(e.target.value) || 2160;
              const newSettings = { ...settings, image_processing: { ...settings.image_processing, max_width } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("image_processing", settings.image_processing)}
            className="w-32"
          />
        </SettingsRow>
      </SettingsCard>

      {/* Cache Settings */}
      <SettingsCard icon="cache" title={t("settings.cache")} description={t("settings.cacheDesc")}>
        <div className="settings-well grid grid-cols-2 gap-4 rounded-xl p-4">
          <div>
            <p className="text-xs text-muted-foreground">{t("settings.diskCacheSize")}</p>
            <p className="mt-0.5 text-xl font-semibold tabular-nums">{cacheStats.total_size_mb.toFixed(2)} MB</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t("settings.files")}</p>
            <p className="mt-0.5 text-xl font-semibold tabular-nums">{cacheStats.file_count}</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t("settings.memoryCacheSize")}</p>
            <p className="mt-0.5 text-xl font-semibold tabular-nums">{cacheStats.memory_size_mb.toFixed(2)} / {cacheStats.memory_max_size_mb.toFixed(0)} MB</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t("settings.cachedPages")}</p>
            <p className="mt-0.5 text-xl font-semibold tabular-nums">{cacheStats.memory_page_count}</p>
          </div>
          <div className="col-span-2">
            <p className="text-xs text-muted-foreground">{t("settings.directory")}</p>
            <p className="mt-0.5 truncate font-mono text-sm" title={cacheStats.directory}>{cacheStats.directory}</p>
          </div>
        </div>

        {clearResult && (
          <div className={`rounded-xl p-3 text-sm ${clearResult.success ? 'bg-success/10 text-success' : 'bg-destructive/10 text-destructive'}`}>
            {clearResult.message}
          </div>
        )}

        <SettingsRow label={t("settings.cacheDirectory")}>
          <FormInput
            value={settings.cache.directory}
            onChange={(e) => {
              const newSettings = { ...settings, cache: { ...settings.cache, directory: e.target.value } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("cache", settings.cache)}
          />
        </SettingsRow>
        <SettingsRow label={t("settings.maxSizeMb")}>
          <FormInput
            type="number"
            value={settings.cache.max_size_mb}
            onChange={(e) => {
              const max_size_mb = parseInt(e.target.value) || 10000;
              const newSettings = { ...settings, cache: { ...settings.cache, max_size_mb } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("cache", settings.cache)}
            className="w-40"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.memoryMaxSizeMb")} description={t("settings.memoryMaxSizeMbHelp")}>
          <FormInput
            type="number"
            min={1}
            value={settings.cache.memory_max_size_mb}
            onChange={(e) => {
              const memory_max_size_mb = Math.max(1, parseInt(e.target.value) || 128);
              setSettings({ ...settings, cache: { ...settings.cache, memory_max_size_mb } });
            }}
            onBlur={() => handleUpdateSetting("cache", settings.cache)}
            className="w-40"
          />
        </SettingsRow>

        <div className="pt-1">
          <Button
            onClick={handleClearCache}
            disabled={isClearing}
            variant="destructive"
          >
            {isClearing ? (
              <>
                <Icon name="spinner" size="sm" className="animate-spin -ml-1 mr-2" />
                {t("settings.clearing")}
              </>
            ) : (
              <>
                <Icon name="trash" size="sm" className="mr-2" />
                {t("settings.clearCache")}
              </>
            )}
          </Button>
        </div>
      </SettingsCard>

      {/* Thumbnail Settings */}
      <SettingsCard icon="image" title={t("settings.thumbnails")} description={t("settings.thumbnailsDesc")}>
        <SettingsRow label={t("settings.enableThumbnails")}>
          <FormSelect
            value={settings.thumbnail.enabled ? "true" : "false"}
            onChange={(e) => {
              const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, enabled: e.target.value === "true" } };
              setSettings(newSettings);
              handleUpdateSetting("thumbnail", newSettings.thumbnail);
            }}
          >
            <option value="true">{t("common.enabled")}</option>
            <option value="false">{t("common.disabled")}</option>
          </FormSelect>
        </SettingsRow>
        <SettingsRow
          label={t("settings.outputFormat")}
          description={
            settings.thumbnail.format === "original"
              ? t("settings.formatOriginalDesc")
              : t("settings.formatReencodeDesc")
          }
        >
          <FormSelect
            value={settings.thumbnail.format}
            onChange={(e) => {
              const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, format: e.target.value } };
              setSettings(newSettings);
              handleUpdateSetting("thumbnail", newSettings.thumbnail);
            }}
          >
            <option value="original">{t("settings.formatOriginal")}</option>
            <option value="webp">WebP</option>
            <option value="jpeg">JPEG</option>
            <option value="png">PNG</option>
          </FormSelect>
        </SettingsRow>
        <SettingsRow label={t("settings.width")}>
          <FormInput
            type="number"
            min={50}
            max={600}
            value={settings.thumbnail.width}
            onChange={(e) => {
              const width = parseInt(e.target.value) || 300;
              const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, width } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("thumbnail", settings.thumbnail)}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.height")}>
          <FormInput
            type="number"
            min={50}
            max={800}
            value={settings.thumbnail.height}
            onChange={(e) => {
              const height = parseInt(e.target.value) || 400;
              const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, height } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("thumbnail", settings.thumbnail)}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.quality")}>
          <FormInput
            type="number"
            min={1}
            max={100}
            value={settings.thumbnail.quality}
            onChange={(e) => {
              const quality = parseInt(e.target.value) || 80;
              const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, quality } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("thumbnail", settings.thumbnail)}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.thumbnailDirectory")}>
          <FormInput
            value={settings.thumbnail.directory}
            onChange={(e) => {
              const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, directory: e.target.value } };
              setSettings(newSettings);
            }}
            onBlur={() => handleUpdateSetting("thumbnail", settings.thumbnail)}
          />
        </SettingsRow>

        <div className="settings-well grid grid-cols-2 gap-4 rounded-xl p-4 sm:grid-cols-3">
          <div>
            <p className="text-xs text-muted-foreground">{t("settings.totalSize")}</p>
            <p className="mt-0.5 text-xl font-semibold tabular-nums">{thumbnailStats.total_size_mb.toFixed(2)} MB</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t("settings.files")}</p>
            <p className="mt-0.5 text-xl font-semibold tabular-nums">{thumbnailStats.file_count}</p>
          </div>
          <div className="col-span-2 sm:col-span-1">
            <p className="text-xs text-muted-foreground">{t("settings.directory")}</p>
            <p className="mt-0.5 truncate font-mono text-sm" title={thumbnailStats.directory}>{thumbnailStats.directory}</p>
          </div>
        </div>

        <p className="settings-well flex items-start gap-2.5 rounded-xl p-3.5 text-sm leading-relaxed text-muted-foreground">
          <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-primary" />
          {t("settings.thumbnailsNote")}
        </p>
      </SettingsCard>
      </SettingsSections>)}

      {activeTab === "metadata" && (<SettingsSections>
      {/* Metadata Providers */}
      <MetadataProvidersCard handleUpdateSetting={handleUpdateSetting} initialData={initialMetadataProviders} />


      {/* Status Mappings */}
      <StatusMappingsCard initialStatusMappings={initialStatusMappings} initialSeriesStatuses={initialSeriesStatuses} initialProviderStatuses={initialProviderStatuses} />
      </SettingsSections>)}

      {activeTab === "ai" && (<SettingsSections layout="stack"><AiTaggingCard handleUpdateSetting={handleUpdateSetting} initialData={settings.ai_tagging ?? null} /></SettingsSections>)}

      {activeTab === "downloadTools" && (<SettingsSections>
      {/* Downloads feature toggle */}
      <SettingsCard
        icon="play"
        title={t("settings.downloadsEnabled")}
        description={t("settings.downloadsEnabledDesc")}
        action={
          <Switch
            checked={downloadsEnabled}
            onChange={async () => {
              const next = !downloadsEnabled;
              setDownloadsEnabled(next);
              try {
                const resp = await fetch("/api/settings/downloads_enabled", {
                  method: "POST",
                  headers: { "Content-Type": "application/json" },
                  body: JSON.stringify({ value: { enabled: next } }),
                });
                if (resp.ok) {
                  toast(t("settings.savedSuccess"), "success");
                  router.refresh();
                } else {
                  setDownloadsEnabled(!next);
                  toast(t("settings.savedError"), "error");
                }
              } catch {
                setDownloadsEnabled(!next);
                toast(t("settings.saveError"), "error");
              }
            }}
          />
        }
      />

      {/* Prowlarr */}
      <ProwlarrCard handleUpdateSetting={handleUpdateSetting} initialData={initialProwlarr} />

      {/* Download client choice — qBittorrent and AllDebrid are alternatives, not cumulative */}
      <p className="settings-well flex items-start gap-2.5 rounded-xl p-3.5 text-sm leading-relaxed text-muted-foreground">
        <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-primary" />
        {t("settings.downloadClientHelp")}
      </p>

      {/* qBittorrent */}
      <QBittorrentCard handleUpdateSetting={handleUpdateSetting} initialQbittorrent={initialQbittorrent} initialTorrentImport={initialTorrentImport} />

      {/* AllDebrid */}
      <AlldebridCard handleUpdateSetting={handleUpdateSetting} initialData={initialAlldebrid} />

      {/* Telegram Monitor */}
      <TelegramMonitorCard
        initialLibraries={initialLibraries}
        initialConcurrentDownloads={settings.limits.concurrent_telegram_downloads ?? 2}
        onSaveConcurrentDownloads={(value) => {
          const newLimits = { ...settings.limits, concurrent_telegram_downloads: value };
          setSettings({ ...settings, limits: newLimits });
          handleUpdateSetting("limits", newLimits);
        }}
        initialDownloadConnections={settings.limits.telegram_download_connections ?? 4}
        onSaveDownloadConnections={(value) => {
          const newLimits = { ...settings.limits, telegram_download_connections: value };
          setSettings({ ...settings, limits: newLimits });
          handleUpdateSetting("limits", newLimits);
        }}
      />

      {/* Direct download providers (JSON / HTML catalogues) */}
      <DdlProvidersCard />
      </SettingsSections>)}

      {activeTab === "notifications" && (<SettingsSections layout="stack">
      {/* Telegram Notifications */}
      <TelegramCard handleUpdateSetting={handleUpdateSetting} initialData={initialTelegram} />
      </SettingsSections>)}

      {activeTab === "readingStatus" && (<SettingsSections>
        <AnilistTab handleUpdateSetting={handleUpdateSetting} users={users} initialData={initialAnilist} />
        <KomgaSyncCard users={users} initialData={initialKomga} />
      </SettingsSections>)}

      {activeTab === "archives" && (
        <SettingsSections layout="stack">
          <ArchivesTab />
        </SettingsSections>
      )}

      {activeTab === "tokens" && <SettingsSections layout="stack">{tokensContent}</SettingsSections>}
    </div>
  );
}

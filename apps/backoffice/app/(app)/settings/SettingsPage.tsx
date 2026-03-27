"use client";

import { useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, FormRow, Icon, toast, Toaster } from "@/app/components/ui";
import { Settings, CacheStats, ClearCacheResponse, ThumbnailStats, UserDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import type { Locale } from "@/lib/i18n/types";
import { MetadataProvidersCard } from "./components/MetadataProvidersCard";
import { StatusMappingsCard } from "./components/StatusMappingsCard";
import { ProwlarrCard } from "./components/ProwlarrCard";
import { QBittorrentCard } from "./components/QBittorrentCard";
import { TelegramCard } from "./components/TelegramCard";
import { KomgaSyncCard } from "./components/KomgaSyncCard";
import { AnilistTab } from "./components/AnilistTab";

interface SettingsPageProps {
  initialSettings: Settings;
  initialCacheStats: CacheStats;
  initialThumbnailStats: ThumbnailStats;
  users: UserDto[];
  initialTab?: string;
  initialProwlarr: Record<string, unknown> | null;
  initialQbittorrent: Record<string, unknown> | null;
  initialTorrentImport: Record<string, unknown> | null;
  initialTelegram: Record<string, unknown> | null;
  initialAnilist: Record<string, unknown> | null;
  initialKomga: Record<string, unknown> | null;
  initialMetadataProviders: Record<string, unknown> | null;
  initialStatusMappings: Record<string, unknown>[];
  initialSeriesStatuses: string[];
  initialProviderStatuses: string[];
  versions?: { api: string; indexer: string; backoffice: string };
}

export default function SettingsPage({ initialSettings, initialCacheStats, initialThumbnailStats, users, initialTab, initialProwlarr, initialQbittorrent, initialTorrentImport, initialTelegram, initialAnilist, initialKomga, initialMetadataProviders, initialStatusMappings, initialSeriesStatuses, initialProviderStatuses, versions }: SettingsPageProps) {
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

  const VALID_TABS = ["general", "downloadTools", "metadata", "readingStatus", "notifications"] as const;
  type TabId = typeof VALID_TABS[number];

  function resolveTab(tab: string | null | undefined): TabId {
    if (tab && (VALID_TABS as readonly string[]).includes(tab)) return tab as TabId;
    return "general";
  }

  const [activeTab, setActiveTab] = useState<TabId>(
    resolveTab(searchParams.get("tab") ?? initialTab)
  );

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

  const tabs = [
    { id: "general" as const, label: t("settings.general"), icon: "settings" as const },
    { id: "downloadTools" as const, label: t("settings.downloadTools"), icon: "play" as const },
    { id: "metadata" as const, label: t("settings.metadata"), icon: "tag" as const },
    { id: "readingStatus" as const, label: t("settings.readingStatus"), icon: "eye" as const },
    { id: "notifications" as const, label: t("settings.notifications"), icon: "bell" as const },
  ];

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="settings" size="xl" />
          {t("settings.title")}
        </h1>
      </div>

      {/* Tab Navigation */}
      <div className="flex gap-1 mb-6 border-b border-border overflow-x-auto scrollbar-none">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => handleTabChange(tab.id)}
            className={`flex items-center gap-2 px-3 sm:px-4 py-2.5 text-xs sm:text-sm font-medium border-b-2 transition-colors -mb-px whitespace-nowrap ${
              activeTab === tab.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
            }`}
          >
            <Icon name={tab.icon} size="sm" />
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === "general" && (<>
      {/* Language Selector */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="settings" size="md" />
            {t("settings.language")}
          </CardTitle>
          <CardDescription>{t("settings.languageDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <FormSelect
            value={locale}
            onChange={(e) => setLocale(e.target.value as Locale)}
          >
            <option value="fr">Français</option>
            <option value="en">English</option>
          </FormSelect>
        </CardContent>
      </Card>

      {/* Image Processing Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="image" size="md" />
            {t("settings.imageProcessing")}
          </CardTitle>
          <CardDescription><span dangerouslySetInnerHTML={{ __html: t("settings.imageProcessingDesc") }} /></CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.defaultFormat")}</label>
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
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.defaultQuality")}</label>
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
                />
              </FormField>
            </FormRow>
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.defaultFilter")}</label>
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
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.maxWidth")}</label>
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
                />
              </FormField>
            </FormRow>
          </div>
        </CardContent>
      </Card>

      {/* Cache Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="cache" size="md" />
            {t("settings.cache")}
          </CardTitle>
          <CardDescription>{t("settings.cacheDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="grid grid-cols-3 gap-4 p-4 bg-muted/30 rounded-lg">
              <div>
                <p className="text-sm text-muted-foreground">{t("settings.cacheSize")}</p>
                <p className="text-2xl font-semibold">{cacheStats.total_size_mb.toFixed(2)} MB</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">{t("settings.files")}</p>
                <p className="text-2xl font-semibold">{cacheStats.file_count}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">{t("settings.directory")}</p>
                <p className="text-sm font-mono truncate" title={cacheStats.directory}>{cacheStats.directory}</p>
              </div>
            </div>

            {clearResult && (
              <div className={`p-3 rounded-lg ${clearResult.success ? 'bg-success/10 text-success' : 'bg-destructive/10 text-destructive'}`}>
                {clearResult.message}
              </div>
            )}

            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.cacheDirectory")}</label>
                <FormInput
                  value={settings.cache.directory}
                  onChange={(e) => {
                    const newSettings = { ...settings, cache: { ...settings.cache, directory: e.target.value } };
                    setSettings(newSettings);
                  }}
                  onBlur={() => handleUpdateSetting("cache", settings.cache)}
                />
              </FormField>
              <FormField className="w-32">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.maxSizeMb")}</label>
                <FormInput
                  type="number"
                  value={settings.cache.max_size_mb}
                  onChange={(e) => {
                    const max_size_mb = parseInt(e.target.value) || 10000;
                    const newSettings = { ...settings, cache: { ...settings.cache, max_size_mb } };
                    setSettings(newSettings);
                  }}
                  onBlur={() => handleUpdateSetting("cache", settings.cache)}
                />
              </FormField>
            </FormRow>

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
        </CardContent>
      </Card>

      {/* Limits Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="performance" size="md" />
            {t("settings.performanceLimits")}
          </CardTitle>
          <CardDescription>{t("settings.performanceDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.concurrentRenders")}</label>
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
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t("settings.concurrentRendersHelp")}
                </p>
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.timeoutSeconds")}</label>
                <FormInput
                  type="number"
                  min={5}
                  max={60}
                  value={settings.limits.timeout_seconds}
                  onChange={(e) => {
                    const timeout_seconds = parseInt(e.target.value) || 12;
                    const newSettings = { ...settings, limits: { ...settings.limits, timeout_seconds } };
                    setSettings(newSettings);
                  }}
                  onBlur={() => handleUpdateSetting("limits", settings.limits)}
                />
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.rateLimit")}</label>
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
                />
              </FormField>
            </FormRow>
            <p className="text-sm text-muted-foreground">
              {t("settings.limitsNote")}
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Thumbnail Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="image" size="md" />
            {t("settings.thumbnails")}
          </CardTitle>
          <CardDescription>{t("settings.thumbnailsDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.enableThumbnails")}</label>
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
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.outputFormat")}</label>
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
                <p className="text-xs text-muted-foreground mt-1">
                  {settings.thumbnail.format === "original"
                    ? t("settings.formatOriginalDesc")
                    : t("settings.formatReencodeDesc")}
                </p>
              </FormField>
            </FormRow>
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.width")}</label>
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
                />
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.height")}</label>
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
                />
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.quality")}</label>
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
                />
              </FormField>
            </FormRow>
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.thumbnailDirectory")}</label>
                <FormInput
                  value={settings.thumbnail.directory}
                  onChange={(e) => {
                    const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, directory: e.target.value } };
                    setSettings(newSettings);
                  }}
                  onBlur={() => handleUpdateSetting("thumbnail", settings.thumbnail)}
                />
              </FormField>
            </FormRow>

            <div className="grid grid-cols-3 gap-4 p-4 bg-muted/30 rounded-lg">
              <div>
                <p className="text-sm text-muted-foreground">{t("settings.totalSize")}</p>
                <p className="text-2xl font-semibold">{thumbnailStats.total_size_mb.toFixed(2)} MB</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">{t("settings.files")}</p>
                <p className="text-2xl font-semibold">{thumbnailStats.file_count}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">{t("settings.directory")}</p>
                <p className="text-sm font-mono truncate" title={thumbnailStats.directory}>{thumbnailStats.directory}</p>
              </div>
            </div>

            <p className="text-sm text-muted-foreground">
              {t("settings.thumbnailsNote")}
            </p>
          </div>
        </CardContent>
      </Card>

      {/* About */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="document" size="md" />
            {t("settings.about")}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div>
              <h3 className="text-lg font-semibold text-foreground">Stripstream Librarian</h3>
              <p className="text-sm text-muted-foreground mt-1">{t("settings.aboutDesc")}</p>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div className="flex items-center gap-3 p-3 rounded-lg bg-muted/30 border border-border/40">
                <div className="w-8 h-8 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
                  <Icon name="settings" size="sm" className="text-primary" />
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">API</p>
                  <p className="text-sm font-mono font-medium text-foreground">{versions?.api ?? "?"}</p>
                </div>
              </div>
              <div className="flex items-center gap-3 p-3 rounded-lg bg-muted/30 border border-border/40">
                <div className="w-8 h-8 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
                  <Icon name="jobs" size="sm" className="text-primary" />
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">Indexer</p>
                  <p className="text-sm font-mono font-medium text-foreground">{versions?.indexer ?? "?"}</p>
                </div>
              </div>
              <div className="flex items-center gap-3 p-3 rounded-lg bg-muted/30 border border-border/40">
                <div className="w-8 h-8 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
                  <Icon name="books" size="sm" className="text-primary" />
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">Backoffice</p>
                  <p className="text-sm font-mono font-medium text-foreground">{versions?.backoffice ?? "?"}</p>
                </div>
              </div>
            </div>

            <div className="flex items-center gap-4 pt-2 text-sm">
              <a
                href="https://github.com/julienfroidefond/stripstream-librarian"
                target="_blank"
                rel="noopener noreferrer"
                className="text-primary hover:underline flex items-center gap-1.5"
              >
                <Icon name="externalLink" size="sm" />
                GitHub
              </a>
            </div>
          </div>
        </CardContent>
      </Card>

      </>)}

      {activeTab === "metadata" && (<>
      {/* Metadata Providers */}
      <MetadataProvidersCard handleUpdateSetting={handleUpdateSetting} initialData={initialMetadataProviders} />

      {/* Status Mappings */}
      <StatusMappingsCard initialStatusMappings={initialStatusMappings} initialSeriesStatuses={initialSeriesStatuses} initialProviderStatuses={initialProviderStatuses} />
      </>)}

      {activeTab === "downloadTools" && (<>
      {/* Prowlarr */}
      <ProwlarrCard handleUpdateSetting={handleUpdateSetting} initialData={initialProwlarr} />

      {/* qBittorrent */}
      <QBittorrentCard handleUpdateSetting={handleUpdateSetting} initialQbittorrent={initialQbittorrent} initialTorrentImport={initialTorrentImport} />
      </>)}

      {activeTab === "notifications" && (<>
      {/* Telegram Notifications */}
      <TelegramCard handleUpdateSetting={handleUpdateSetting} initialData={initialTelegram} />
      </>)}

      {activeTab === "readingStatus" && (<>
        <AnilistTab handleUpdateSetting={handleUpdateSetting} users={users} initialData={initialAnilist} />
        <KomgaSyncCard users={users} initialData={initialKomga} />
      </>)}

      <Toaster />
    </>
  );
}

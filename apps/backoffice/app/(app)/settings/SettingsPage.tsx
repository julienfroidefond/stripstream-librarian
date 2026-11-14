"use client";

import { useEffect, useState, type ReactNode } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Button, FormInput, FormSelect, Icon, SettingsCard, SettingsColumn, SettingsColumns, SettingsPageHeader, SettingsRow, SettingsSection, SettingsSections, SettingsTabs, Switch, type SettingsTabItem } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import type { CacheStats, ClearCacheResponse } from "@/lib/api";
import { browserApi } from "@/lib/api/browser";
import { useTranslation } from "@/lib/i18n/context";
import type { Locale } from "@/lib/i18n/types";
import type { SettingsPageModel } from "@/lib/settings/page";
import type { CacheSettings, DownloadsEnabledSettings, ImageProcessingSettings, LimitsSettings, ThumbnailSettings } from "@/lib/settings/sections";
import { SettingsDraftProvider, useSettingsReadOnly, useSettingsSection, useSettingsSaving } from "./SettingsDraftProvider";
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
import { OrphansTab } from "./components/OrphansTab";
import { TelegramMonitorCard } from "./components/TelegramMonitorCard";
import { DdlProvidersCard } from "./components/DdlProvidersCard";
import { DdlEngineCard } from "./components/DdlEngineCard";
import { AiTaggingCard } from "./components/AiTaggingCard";
import { DownloadClientCard } from "./components/DownloadClientCard";

interface SettingsPageProps {
  /** Everything loaded by `loadSettingsPage`, with its degraded sections. */
  model: SettingsPageModel;
  initialTab?: string;
  tokensContent?: ReactNode;
}

/**
 * Settings shell: the provider owns every `app_settings` section, the content
 * is a set of views over slices. Keeping the provider above the content means
 * a card that is not mounted (inactive tab) still keeps its unsaved edits.
 */
export default function SettingsPage(props: SettingsPageProps) {
  // `settings === null`: the stored values were not read, so the provider
  // seeds defaults for display only and refuses every save.
  return (
    <SettingsDraftProvider initial={props.model.settings}>
      <SettingsPageContent {...props} />
    </SettingsDraftProvider>
  );
}

function SettingsPageContent({ model, initialTab, tokensContent }: SettingsPageProps) {
  const { users, versions } = model;
  const { t, locale, setLocale } = useTranslation();
  const router = useRouter();
  const searchParams = useSearchParams();

  const limits = useSettingsSection<LimitsSettings>("limits");
  const imageProcessing = useSettingsSection<ImageProcessingSettings>("image_processing");
  const cache = useSettingsSection<CacheSettings>("cache");
  const thumbnail = useSettingsSection<ThumbnailSettings>("thumbnail");
  const downloadsEnabled = useSettingsSection<DownloadsEnabledSettings>("downloads_enabled");
  const isSaving = useSettingsSaving();
  const readOnly = useSettingsReadOnly();

  // `null` = the API did not answer: render "unavailable", never a made-up path.
  const [cacheStats, setCacheStats] = useState<CacheStats | null>(model.cacheStats);
  const thumbnailStats = model.thumbnailStats;
  const [isClearing, setIsClearing] = useState(false);
  const [clearResult, setClearResult] = useState<ClearCacheResponse | null>(null);

  const VALID_TABS = ["general", "media", "downloadTools", "metadata", "ai", "readingStatus", "notifications", "tokens", "archives", "orphans"] as const;
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
      try {
        const stats = await browserApi.settings.cacheStats();
        if (active) setCacheStats(stats);
      } catch {
        // Keep the last known stats while the API restarts.
      }
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

  async function handleDownloadsToggle() {
    downloadsEnabled.patch({ enabled: !downloadsEnabled.value.enabled });
    const result = await downloadsEnabled.save();
    if (result.status === "saved") router.refresh();
  }

  async function handleClearCache() {
    setIsClearing(true);
    setClearResult(null);
    try {
      const result = await browserApi.settings.clearCache();
      setClearResult(result);
      try {
        setCacheStats(await browserApi.settings.cacheStats());
      } catch {
        // The purge succeeded; keep the previous stats rather than reporting failure.
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
    { id: "orphans", label: t("settings.orphans"), icon: "books" },
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

      {readOnly && (
        <div
          role="alert"
          className="mb-6 flex items-start gap-2.5 rounded-xl bg-warning/10 p-3.5 text-sm leading-relaxed text-warning"
        >
          <Icon name="warning" size="sm" mono className="mt-0.5 shrink-0" />
          <div>
            <p className="font-medium">{t("settings.readOnlyTitle")}</p>
            <p>{t("settings.readOnlyDesc")}</p>
          </div>
        </div>
      )}

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
            value={limits.value.concurrent_renders}
            onChange={(e) => limits.patch({ concurrent_renders: parseInt(e.target.value) || 4 })}
            onBlur={() => void limits.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.timeoutSeconds")}>
          <FormInput
            type="number"
            min={5}
            max={300}
            value={limits.value.timeout_seconds}
            onChange={(e) => limits.patch({ timeout_seconds: parseInt(e.target.value) || 12 })}
            onBlur={() => void limits.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.analysisTimeoutSeconds")} description={t("settings.analysisTimeoutSecondsHelp")}>
          <FormInput
            type="number"
            min={10}
            max={3600}
            value={limits.value.analysis_timeout_seconds}
            onChange={(e) => limits.patch({ analysis_timeout_seconds: parseInt(e.target.value) || 120 })}
            onBlur={() => void limits.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.rateLimit")}>
          <FormInput
            type="number"
            min={10}
            max={1000}
            value={limits.value.rate_limit_per_second}
            onChange={(e) => limits.patch({ rate_limit_per_second: parseInt(e.target.value) || 120 })}
            onBlur={() => void limits.save()}
            className="w-32"
          />
        </SettingsRow>
        <p className="settings-well flex items-start gap-2.5 rounded-xl p-3.5 text-sm leading-relaxed text-muted-foreground">
          <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-primary" />
          {t("settings.limitsNote")}
        </p>
      </SettingsCard>

      {/* Rename Format */}
      <RenameFormatCard />

      {/* About */}
      <SettingsCard icon="document" title={t("settings.about")}>
        <div>
          <p className="text-sm font-medium text-foreground">Stripstream Librarian</p>
          <p className="mt-0.5 text-sm leading-relaxed text-muted-foreground">{t("settings.aboutDesc")}</p>
        </div>

        <dl className="settings-well grid grid-cols-1 gap-x-6 gap-y-4 rounded-xl p-4 sm:grid-cols-3">
          <div>
            <dt className="text-xs text-muted-foreground">API</dt>
            <dd className="mt-0.5 text-sm font-medium tabular-nums text-foreground">{versions.api ?? "?"}</dd>
          </div>
          <div>
            <dt className="text-xs text-muted-foreground">Indexer</dt>
            <dd className="mt-0.5 text-sm font-medium tabular-nums text-foreground">{versions.indexer ?? "?"}</dd>
          </div>
          <div>
            <dt className="text-xs text-muted-foreground">Backoffice</dt>
            <dd className="mt-0.5 text-sm font-medium tabular-nums text-foreground">{versions.backoffice}</dd>
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
            value={imageProcessing.value.format}
            onChange={(e) => {
              imageProcessing.patch({ format: e.target.value });
              void imageProcessing.save();
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
            value={imageProcessing.value.quality}
            onChange={(e) => imageProcessing.patch({ quality: parseInt(e.target.value) || 85 })}
            onBlur={() => void imageProcessing.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.defaultFilter")}>
          <FormSelect
            value={imageProcessing.value.filter}
            onChange={(e) => {
              imageProcessing.patch({ filter: e.target.value });
              void imageProcessing.save();
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
            value={imageProcessing.value.max_width}
            onChange={(e) => imageProcessing.patch({ max_width: parseInt(e.target.value) || 2160 })}
            onBlur={() => void imageProcessing.save()}
            className="w-32"
          />
        </SettingsRow>
      </SettingsCard>

      {/* Cache Settings */}
      <SettingsCard icon="cache" title={t("settings.cache")} description={t("settings.cacheDesc")}>
        {cacheStats ? (
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
        ) : (
          <p className="settings-well rounded-xl p-4 text-sm text-muted-foreground">{t("settings.statsUnavailable")}</p>
        )}

        {clearResult && (
          <div className={`rounded-xl p-3 text-sm ${clearResult.success ? 'bg-success/10 text-success' : 'bg-destructive/10 text-destructive'}`}>
            {clearResult.message}
          </div>
        )}

        <SettingsRow label={t("settings.cacheDirectory")}>
          <FormInput
            value={cache.value.directory}
            onChange={(e) => cache.patch({ directory: e.target.value })}
            onBlur={() => void cache.save()}
          />
        </SettingsRow>
        <SettingsRow label={t("settings.maxSizeMb")}>
          <FormInput
            type="number"
            value={cache.value.max_size_mb}
            onChange={(e) => cache.patch({ max_size_mb: parseInt(e.target.value) || 10000 })}
            onBlur={() => void cache.save()}
            className="w-40"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.memoryMaxSizeMb")} description={t("settings.memoryMaxSizeMbHelp")}>
          <FormInput
            type="number"
            min={1}
            value={cache.value.memory_max_size_mb}
            onChange={(e) => cache.patch({ memory_max_size_mb: Math.max(1, parseInt(e.target.value) || 128) })}
            onBlur={() => void cache.save()}
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
            value={thumbnail.value.enabled ? "true" : "false"}
            onChange={(e) => {
              thumbnail.patch({ enabled: e.target.value === "true" });
              void thumbnail.save();
            }}
          >
            <option value="true">{t("common.enabled")}</option>
            <option value="false">{t("common.disabled")}</option>
          </FormSelect>
        </SettingsRow>
        <SettingsRow
          label={t("settings.outputFormat")}
          description={
            thumbnail.value.format === "original"
              ? t("settings.formatOriginalDesc")
              : t("settings.formatReencodeDesc")
          }
        >
          <FormSelect
            value={thumbnail.value.format}
            onChange={(e) => {
              thumbnail.patch({ format: e.target.value });
              void thumbnail.save();
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
            value={thumbnail.value.width}
            onChange={(e) => thumbnail.patch({ width: parseInt(e.target.value) || 300 })}
            onBlur={() => void thumbnail.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.height")}>
          <FormInput
            type="number"
            min={50}
            max={800}
            value={thumbnail.value.height}
            onChange={(e) => thumbnail.patch({ height: parseInt(e.target.value) || 400 })}
            onBlur={() => void thumbnail.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.quality")}>
          <FormInput
            type="number"
            min={1}
            max={100}
            value={thumbnail.value.quality}
            onChange={(e) => thumbnail.patch({ quality: parseInt(e.target.value) || 80 })}
            onBlur={() => void thumbnail.save()}
            className="w-32"
          />
        </SettingsRow>
        <SettingsRow label={t("settings.thumbnailDirectory")}>
          <FormInput
            value={thumbnail.value.directory}
            onChange={(e) => thumbnail.patch({ directory: e.target.value })}
            onBlur={() => void thumbnail.save()}
          />
        </SettingsRow>

        {thumbnailStats ? (
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
        ) : (
          <p className="settings-well rounded-xl p-4 text-sm text-muted-foreground">{t("settings.statsUnavailable")}</p>
        )}

        <p className="settings-well flex items-start gap-2.5 rounded-xl p-3.5 text-sm leading-relaxed text-muted-foreground">
          <Icon name="info" size="sm" mono className="mt-0.5 shrink-0 text-primary" />
          {t("settings.thumbnailsNote")}
        </p>
      </SettingsCard>
      </SettingsSections>)}

      {activeTab === "metadata" && (<SettingsSections>
      {/* Metadata Providers */}
      <MetadataProvidersCard />


      {/* Status Mappings */}
      <StatusMappingsCard initialStatusMappings={model.statusMappings} initialSeriesStatuses={model.seriesStatuses} initialProviderStatuses={model.providerStatuses} />
      </SettingsSections>)}

      {activeTab === "ai" && (<SettingsSections layout="stack"><AiTaggingCard /></SettingsSections>)}

      {activeTab === "downloadTools" && (
        <div className="space-y-8">
          {/* Master switch: it gates the whole tab, so it sits above the sections. */}
          <SettingsCard
            icon="play"
            title={t("settings.downloadsEnabled")}
            description={t("settings.downloadsEnabledDesc")}
            action={
              <Switch
                checked={downloadsEnabled.value.enabled}
                onChange={() => void handleDownloadsToggle()}
              />
            }
          />

          {/* Where releases are searched */}
          <SettingsSection title={t("settings.sectionProvider")}>
            <ProwlarrCard />
          </SettingsSection>

          {/* Where they are handed over: the choice is made once, then the two alternatives */}
          <SettingsSection title={t("settings.sectionDownloadClients")}>
            <DownloadClientCard />
            <SettingsColumns>
              <SettingsColumn>
                <QBittorrentCard />
              </SettingsColumn>
              <SettingsColumn>
                <AlldebridCard />
              </SettingsColumn>
            </SettingsColumns>
          </SettingsSection>

          {/* How the transfers themselves are tuned (DDL concurrency + shared knobs) */}
          <SettingsSection title={t("settings.sectionDownloadEngine")}>
            <DdlEngineCard />
          </SettingsSection>

          {/* Direct download providers (JSON / HTML catalogues) */}
          <SettingsSection title={t("settings.sectionDdl")}>
            <DdlProvidersCard />
          </SettingsSection>

          {/* Telegram link monitoring */}
          <SettingsSection title={t("settings.sectionTelegram")}>
            <TelegramMonitorCard initialLibraries={model.libraries} />
          </SettingsSection>
        </div>
      )}

      {activeTab === "notifications" && (<SettingsSections layout="stack">
      {/* Telegram Notifications */}
      <TelegramCard />
      </SettingsSections>)}

      {activeTab === "readingStatus" && (<SettingsSections>
        <AnilistTab users={users} />
        <KomgaSyncCard users={users} />
      </SettingsSections>)}

      {activeTab === "archives" && (
        <SettingsSections layout="stack">
          <ArchivesTab />
        </SettingsSections>
      )}

      {activeTab === "orphans" && (
        <SettingsSections layout="stack">
          <OrphansTab />
        </SettingsSections>
      )}

      {activeTab === "tokens" && <SettingsSections layout="stack">{tokensContent}</SettingsSections>}
    </div>
  );
}

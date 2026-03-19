"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, FormRow, Icon } from "../components/ui";
import { ProviderIcon } from "../components/ProviderIcon";
import { Settings, CacheStats, ClearCacheResponse, ThumbnailStats, KomgaSyncResponse, KomgaSyncReportSummary, StatusMappingDto } from "../../lib/api";
import { useTranslation } from "../../lib/i18n/context";
import type { Locale } from "../../lib/i18n/types";

interface SettingsPageProps {
  initialSettings: Settings;
  initialCacheStats: CacheStats;
  initialThumbnailStats: ThumbnailStats;
}

export default function SettingsPage({ initialSettings, initialCacheStats, initialThumbnailStats }: SettingsPageProps) {
  const { t, locale, setLocale } = useTranslation();
  const [settings, setSettings] = useState<Settings>({
    ...initialSettings,
    thumbnail: initialSettings.thumbnail || { enabled: true, width: 300, height: 400, quality: 80, format: "webp", directory: "/data/thumbnails" }
  });
  const [cacheStats, setCacheStats] = useState<CacheStats>(initialCacheStats);
  const [thumbnailStats, setThumbnailStats] = useState<ThumbnailStats>(initialThumbnailStats);
  const [isClearing, setIsClearing] = useState(false);
  const [clearResult, setClearResult] = useState<ClearCacheResponse | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  // Komga sync state — URL and username are persisted in settings
  const [komgaUrl, setKomgaUrl] = useState("");
  const [komgaUsername, setKomgaUsername] = useState("");
  const [komgaPassword, setKomgaPassword] = useState("");
  const [isSyncing, setIsSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<KomgaSyncResponse | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [showUnmatched, setShowUnmatched] = useState(false);
  const [reports, setReports] = useState<KomgaSyncReportSummary[]>([]);
  const [selectedReport, setSelectedReport] = useState<KomgaSyncResponse | null>(null);
  const [showReportUnmatched, setShowReportUnmatched] = useState(false);
  const [showMatchedBooks, setShowMatchedBooks] = useState(false);
  const [showReportMatchedBooks, setShowReportMatchedBooks] = useState(false);

  const syncNewlyMarkedSet = useMemo(
    () => new Set(syncResult?.newly_marked_books ?? []),
    [syncResult?.newly_marked_books],
  );
  const reportNewlyMarkedSet = useMemo(
    () => new Set(selectedReport?.newly_marked_books ?? []),
    [selectedReport?.newly_marked_books],
  );

  async function handleUpdateSetting(key: string, value: unknown) {
    setIsSaving(true);
    setSaveMessage(null);
    try {
      const response = await fetch(`/api/settings/${key}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ value })
      });
      if (response.ok) {
        setSaveMessage(t("settings.savedSuccess"));
        setTimeout(() => setSaveMessage(null), 3000);
      } else {
        setSaveMessage(t("settings.savedError"));
      }
    } catch (error) {
      setSaveMessage(t("settings.saveError"));
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
    } catch (error) {
      setClearResult({ success: false, message: t("settings.cacheClearError") });
    } finally {
      setIsClearing(false);
    }
  }

  const fetchReports = useCallback(async () => {
    try {
      const resp = await fetch("/api/komga/reports");
      if (resp.ok) setReports(await resp.json());
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    fetchReports();
    // Load saved Komga credentials (URL + username only)
    fetch("/api/settings/komga").then(r => r.ok ? r.json() : null).then(data => {
      if (data) {
        if (data.url) setKomgaUrl(data.url);
        if (data.username) setKomgaUsername(data.username);
      }
    }).catch(() => {});
  }, [fetchReports]);

  async function handleViewReport(id: string) {
    setSelectedReport(null);
    setShowReportUnmatched(false);
    setShowReportMatchedBooks(false);
    try {
      const resp = await fetch(`/api/komga/reports/${id}`);
      if (resp.ok) setSelectedReport(await resp.json());
    } catch { /* ignore */ }
  }

  async function handleKomgaSync() {
    setIsSyncing(true);
    setSyncResult(null);
    setSyncError(null);
    setShowUnmatched(false);
    setShowMatchedBooks(false);
    try {
      const response = await fetch("/api/komga/sync", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: komgaUrl, username: komgaUsername, password: komgaPassword }),
      });
      const data = await response.json();
      if (!response.ok) {
        setSyncError(data.error || "Sync failed");
      } else {
        setSyncResult(data);
        fetchReports();
        // Persist URL and username (not password)
        fetch("/api/settings/komga", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ value: { url: komgaUrl, username: komgaUsername } }),
        }).catch(() => {});
      }
    } catch {
      setSyncError("Failed to connect to sync endpoint");
    } finally {
      setIsSyncing(false);
    }
  }

  const [activeTab, setActiveTab] = useState<"general" | "integrations">("general");

  const tabs = [
    { id: "general" as const, label: t("settings.general"), icon: "settings" as const },
    { id: "integrations" as const, label: t("settings.integrations"), icon: "refresh" as const },
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
      <div className="flex gap-1 mb-6 border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex items-center gap-2 px-4 py-2.5 text-sm font-medium border-b-2 transition-colors -mb-px ${
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

      {saveMessage && (
        <Card className="mb-6 border-success/50 bg-success/5">
          <CardContent className="pt-6">
            <p className="text-success">{saveMessage}</p>
          </CardContent>
        </Card>
      )}

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

      </>)}

      {activeTab === "integrations" && (<>
      {/* Metadata Providers */}
      <MetadataProvidersCard handleUpdateSetting={handleUpdateSetting} />

      {/* Status Mappings */}
      <StatusMappingsCard />

      {/* Komga Sync */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="refresh" size="md" />
            {t("settings.komgaSync")}
          </CardTitle>
          <CardDescription>{t("settings.komgaDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.komgaUrl")}</label>
                <FormInput
                  type="url"
                  placeholder="https://komga.example.com"
                  value={komgaUrl}
                  onChange={(e) => setKomgaUrl(e.target.value)}
                />
              </FormField>
            </FormRow>
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.username")}</label>
                <FormInput
                  value={komgaUsername}
                  onChange={(e) => setKomgaUsername(e.target.value)}
                />
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.password")}</label>
                <FormInput
                  type="password"
                  value={komgaPassword}
                  onChange={(e) => setKomgaPassword(e.target.value)}
                />
              </FormField>
            </FormRow>

            <Button
              onClick={handleKomgaSync}
              disabled={isSyncing || !komgaUrl || !komgaUsername || !komgaPassword}
            >
              {isSyncing ? (
                <>
                  <Icon name="spinner" size="sm" className="animate-spin -ml-1 mr-2" />
                  {t("settings.syncing")}
                </>
              ) : (
                <>
                  <Icon name="refresh" size="sm" className="mr-2" />
                  {t("settings.syncReadBooks")}
                </>
              )}
            </Button>

            {syncError && (
              <div className="p-3 rounded-lg bg-destructive/10 text-destructive">
                {syncError}
              </div>
            )}

            {syncResult && (
              <div className="space-y-3">
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 p-4 bg-muted/30 rounded-lg">
                  <div>
                    <p className="text-sm text-muted-foreground">{t("settings.komgaRead")}</p>
                    <p className="text-2xl font-semibold">{syncResult.total_komga_read}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">{t("settings.matched")}</p>
                    <p className="text-2xl font-semibold">{syncResult.matched}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">{t("settings.alreadyRead")}</p>
                    <p className="text-2xl font-semibold">{syncResult.already_read}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">{t("settings.newlyMarked")}</p>
                    <p className="text-2xl font-semibold text-success">{syncResult.newly_marked}</p>
                  </div>
                </div>

                {syncResult.matched_books.length > 0 && (
                  <div>
                    <button
                      type="button"
                      onClick={() => setShowMatchedBooks(!showMatchedBooks)}
                      className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                    >
                      <Icon name={showMatchedBooks ? "chevronDown" : "chevronRight"} size="sm" />
                      {t("settings.matchedBooks", { count: syncResult.matched_books.length, plural: syncResult.matched_books.length !== 1 ? "s" : "" })}
                    </button>
                    {showMatchedBooks && (
                      <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-success/5 rounded-lg text-sm space-y-1">
                        {syncResult.matched_books.map((title, i) => (
                          <p key={i} className="text-foreground truncate flex items-center gap-1.5" title={title}>
                            {syncNewlyMarkedSet.has(title) && (
                              <Icon name="check" size="sm" className="text-success shrink-0" />
                            )}
                            {title}
                          </p>
                        ))}
                      </div>
                    )}
                  </div>
                )}

                {syncResult.unmatched.length > 0 && (
                  <div>
                    <button
                      type="button"
                      onClick={() => setShowUnmatched(!showUnmatched)}
                      className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                    >
                      <Icon name={showUnmatched ? "chevronDown" : "chevronRight"} size="sm" />
                      {t("settings.unmatchedBooks", { count: syncResult.unmatched.length, plural: syncResult.unmatched.length !== 1 ? "s" : "" })}
                    </button>
                    {showUnmatched && (
                      <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                        {syncResult.unmatched.map((title, i) => (
                          <p key={i} className="text-muted-foreground truncate" title={title}>{title}</p>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
            {/* Past reports */}
            {reports.length > 0 && (
              <div className="border-t border-border pt-4">
                <h3 className="text-sm font-medium text-foreground mb-3">{t("settings.syncHistory")}</h3>
                <div className="space-y-2">
                  {reports.map((r) => (
                    <button
                      key={r.id}
                      type="button"
                      onClick={() => handleViewReport(r.id)}
                      className={`w-full text-left p-3 rounded-lg border transition-colors ${
                        selectedReport?.id === r.id
                          ? "border-primary bg-primary/5"
                          : "border-border/60 bg-muted/20 hover:bg-muted/40"
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-sm font-medium text-foreground">
                          {new Date(r.created_at).toLocaleString()}
                        </span>
                        <span className="text-xs text-muted-foreground truncate ml-2" title={r.komga_url}>
                          {r.komga_url}
                        </span>
                      </div>
                      <div className="flex gap-4 mt-1 text-xs text-muted-foreground">
                        <span>{r.total_komga_read} {t("settings.read")}</span>
                        <span>{r.matched} {t("settings.matched").toLowerCase()}</span>
                        <span className="text-success">{r.newly_marked} {t("settings.new")}</span>
                        {r.unmatched_count > 0 && (
                          <span className="text-warning">{r.unmatched_count} {t("settings.unmatched")}</span>
                        )}
                      </div>
                    </button>
                  ))}
                </div>

                {/* Selected report detail */}
                {selectedReport && (
                  <div className="mt-3 space-y-3">
                    <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 p-4 bg-muted/30 rounded-lg">
                      <div>
                        <p className="text-sm text-muted-foreground">{t("settings.komgaRead")}</p>
                        <p className="text-2xl font-semibold">{selectedReport.total_komga_read}</p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">{t("settings.matched")}</p>
                        <p className="text-2xl font-semibold">{selectedReport.matched}</p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">{t("settings.alreadyRead")}</p>
                        <p className="text-2xl font-semibold">{selectedReport.already_read}</p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">{t("settings.newlyMarked")}</p>
                        <p className="text-2xl font-semibold text-success">{selectedReport.newly_marked}</p>
                      </div>
                    </div>

                    {selectedReport.matched_books && selectedReport.matched_books.length > 0 && (
                      <div>
                        <button
                          type="button"
                          onClick={() => setShowReportMatchedBooks(!showReportMatchedBooks)}
                          className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                        >
                          <Icon name={showReportMatchedBooks ? "chevronDown" : "chevronRight"} size="sm" />
                          {t("settings.matchedBooks", { count: selectedReport.matched_books.length, plural: selectedReport.matched_books.length !== 1 ? "s" : "" })}
                        </button>
                        {showReportMatchedBooks && (
                          <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-success/5 rounded-lg text-sm space-y-1">
                            {selectedReport.matched_books.map((title, i) => (
                              <p key={i} className="text-foreground truncate flex items-center gap-1.5" title={title}>
                                {reportNewlyMarkedSet.has(title) && (
                                  <Icon name="check" size="sm" className="text-success shrink-0" />
                                )}
                                {title}
                              </p>
                            ))}
                          </div>
                        )}
                      </div>
                    )}

                    {selectedReport.unmatched.length > 0 && (
                      <div>
                        <button
                          type="button"
                          onClick={() => setShowReportUnmatched(!showReportUnmatched)}
                          className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                        >
                          <Icon name={showReportUnmatched ? "chevronDown" : "chevronRight"} size="sm" />
                          {t("settings.unmatchedBooks", { count: selectedReport.unmatched.length, plural: selectedReport.unmatched.length !== 1 ? "s" : "" })}
                        </button>
                        {showReportUnmatched && (
                          <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                            {selectedReport.unmatched.map((title, i) => (
                              <p key={i} className="text-muted-foreground truncate" title={title}>{title}</p>
                            ))}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
        </CardContent>
      </Card>
      </>)}
    </>
  );
}

// ---------------------------------------------------------------------------
// Metadata Providers sub-component
// ---------------------------------------------------------------------------

const METADATA_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
] as const;

function MetadataProvidersCard({ handleUpdateSetting }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void> }) {
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
                  type="password"
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
                  type="password"
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

// ---------------------------------------------------------------------------
// Status Mappings sub-component
// ---------------------------------------------------------------------------

function StatusMappingsCard() {
  const { t } = useTranslation();
  const [mappings, setMappings] = useState<StatusMappingDto[]>([]);
  const [targetStatuses, setTargetStatuses] = useState<string[]>([]);
  const [providerStatuses, setProviderStatuses] = useState<string[]>([]);
  const [newTargetName, setNewTargetName] = useState("");
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    try {
      const [mRes, sRes, pRes] = await Promise.all([
        fetch("/api/settings/status-mappings").then((r) => r.ok ? r.json() : []),
        fetch("/api/series/statuses").then((r) => r.ok ? r.json() : []),
        fetch("/api/series/provider-statuses").then((r) => r.ok ? r.json() : []),
      ]);
      setMappings(mRes);
      setTargetStatuses(sRes);
      setProviderStatuses(pRes);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadData(); }, [loadData]);

  // Group mappings by target status (only those with a non-null mapped_status)
  const grouped = useMemo(() => {
    const map = new Map<string, StatusMappingDto[]>();
    for (const m of mappings) {
      if (m.mapped_status) {
        const list = map.get(m.mapped_status) || [];
        list.push(m);
        map.set(m.mapped_status, list);
      }
    }
    return map;
  }, [mappings]);

  // Unmapped = mappings with null mapped_status + provider statuses not in status_mappings at all
  const knownProviderStatuses = useMemo(
    () => new Set(mappings.map((m) => m.provider_status)),
    [mappings],
  );
  const unmappedMappings = useMemo(
    () => mappings.filter((m) => !m.mapped_status),
    [mappings],
  );
  const newProviderStatuses = useMemo(
    () => providerStatuses.filter((ps) => !knownProviderStatuses.has(ps)),
    [providerStatuses, knownProviderStatuses],
  );

  // All possible targets = existing statuses from DB + custom ones added locally
  const [customTargets, setCustomTargets] = useState<string[]>([]);
  const allTargets = useMemo(() => {
    const set = new Set([...targetStatuses, ...customTargets]);
    return [...set].sort();
  }, [targetStatuses, customTargets]);

  async function handleAssign(providerStatus: string, targetStatus: string) {
    if (!providerStatus || !targetStatus) return;
    try {
      const res = await fetch("/api/settings/status-mappings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider_status: providerStatus, mapped_status: targetStatus }),
      });
      if (res.ok) {
        const created: StatusMappingDto = await res.json();
        setMappings((prev) => [...prev.filter((m) => m.provider_status !== created.provider_status), created]);
      }
    } catch {
      // ignore
    }
  }

  async function handleUnmap(id: string) {
    try {
      const res = await fetch(`/api/settings/status-mappings/${id}`, { method: "DELETE" });
      if (res.ok) {
        const updated: StatusMappingDto = await res.json();
        setMappings((prev) => prev.map((m) => (m.id === id ? updated : m)));
      }
    } catch {
      // ignore
    }
  }

  function handleCreateTarget() {
    const name = newTargetName.trim().toLowerCase();
    if (!name || allTargets.includes(name)) return;
    setCustomTargets((prev) => [...prev, name]);
    setNewTargetName("");
  }

  function statusLabel(status: string) {
    const key = `seriesStatus.${status}` as Parameters<typeof t>[0];
    const translated = t(key);
    return translated !== key ? translated : status;
  }

  if (loading) {
    return (
      <Card className="mb-6">
        <CardContent><p className="text-muted-foreground py-4">{t("common.loading")}</p></CardContent>
      </Card>
    );
  }

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="settings" size="md" />
          {t("settings.statusMappings")}
        </CardTitle>
        <CardDescription>{t("settings.statusMappingsDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          {/* Create new target status */}
          <div className="flex gap-2 items-center">
            <FormInput
              placeholder={t("settings.newTargetPlaceholder")}
              value={newTargetName}
              onChange={(e) => setNewTargetName(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleCreateTarget(); }}
              className="max-w-[250px]"
            />
            <Button
              onClick={handleCreateTarget}
              disabled={!newTargetName.trim() || allTargets.includes(newTargetName.trim().toLowerCase())}
            >
              <Icon name="plus" size="sm" />
              {t("settings.createTargetStatus")}
            </Button>
          </div>

          {/* Grouped by target status */}
          {allTargets.map((target) => {
            const items = grouped.get(target) || [];
            return (
              <div key={target} className="border border-border/50 rounded-lg p-3">
                <div className="flex items-center gap-2 mb-2">
                  <span className="text-sm font-medium text-foreground">
                    {statusLabel(target)}
                  </span>
                  <span className="text-xs text-muted-foreground font-mono">({target})</span>
                </div>
                <div className="flex flex-wrap gap-2">
                  {items.map((m) => (
                    <span
                      key={m.id}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-muted/50 text-sm font-mono"
                    >
                      {m.provider_status}
                      <button
                        type="button"
                        onClick={() => handleUnmap(m.id)}
                        className="ml-1 text-muted-foreground hover:text-destructive transition-colors"
                        title={t("common.delete")}
                      >
                        <Icon name="x" size="sm" />
                      </button>
                    </span>
                  ))}
                  {items.length === 0 && (
                    <span className="text-xs text-muted-foreground italic">{t("settings.noMappings")}</span>
                  )}
                </div>
              </div>
            );
          })}

          {/* Unmapped provider statuses (null mapped_status + brand new from providers) */}
          {(unmappedMappings.length > 0 || newProviderStatuses.length > 0) && (
            <div className="border-t border-border/50 pt-4">
              <h4 className="text-sm font-medium text-foreground mb-3">{t("settings.unmappedSection")}</h4>
              <div className="space-y-2">
                {unmappedMappings.map((m) => (
                  <div key={m.id} className="flex items-center gap-2">
                    <span className="text-sm font-mono bg-muted/50 px-2 py-1 rounded-md min-w-[120px]">{m.provider_status}</span>
                    <Icon name="chevronRight" size="sm" />
                    <FormSelect
                      className="w-auto"
                      value=""
                      onChange={(e) => { if (e.target.value) handleAssign(m.provider_status, e.target.value); }}
                    >
                      <option value="">{t("settings.selectTargetStatus")}</option>
                      {allTargets.map((s) => (
                        <option key={s} value={s}>{statusLabel(s)}</option>
                      ))}
                    </FormSelect>
                  </div>
                ))}
                {newProviderStatuses.map((ps) => (
                  <div key={ps} className="flex items-center gap-2">
                    <span className="text-sm font-mono bg-muted/50 px-2 py-1 rounded-md min-w-[120px]">{ps}</span>
                    <Icon name="chevronRight" size="sm" />
                    <FormSelect
                      className="w-auto"
                      value=""
                      onChange={(e) => { if (e.target.value) handleAssign(ps, e.target.value); }}
                    >
                      <option value="">{t("settings.selectTargetStatus")}</option>
                      {allTargets.map((s) => (
                        <option key={s} value={s}>{statusLabel(s)}</option>
                      ))}
                    </FormSelect>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

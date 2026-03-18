"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, FormRow, Icon } from "../components/ui";
import { ProviderIcon } from "../components/ProviderIcon";
import { Settings, CacheStats, ClearCacheResponse, ThumbnailStats, KomgaSyncResponse, KomgaSyncReportSummary } from "../../lib/api";

interface SettingsPageProps {
  initialSettings: Settings;
  initialCacheStats: CacheStats;
  initialThumbnailStats: ThumbnailStats;
}

export default function SettingsPage({ initialSettings, initialCacheStats, initialThumbnailStats }: SettingsPageProps) {
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
        setSaveMessage("Paramètres enregistrés avec succès");
        setTimeout(() => setSaveMessage(null), 3000);
      } else {
        setSaveMessage("Échec de l'enregistrement des paramètres");
      }
    } catch (error) {
      setSaveMessage("Erreur lors de l'enregistrement des paramètres");
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
      setClearResult({ success: false, message: "Échec du vidage du cache" });
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
    { id: "general" as const, label: "Général", icon: "settings" as const },
    { id: "integrations" as const, label: "Intégrations", icon: "refresh" as const },
  ];

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="settings" size="xl" />
          Paramètres
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
      {/* Image Processing Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="image" size="md" />
            Traitement d&apos;images
          </CardTitle>
          <CardDescription>Ces paramètres s&apos;appliquent uniquement lorsqu&apos;un client demande explicitement une conversion de format via l&apos;API (ex. <code className="text-xs bg-muted px-1 rounded">?format=webp&amp;width=800</code>). Les pages servies sans paramètres sont livrées telles quelles depuis l&apos;archive, sans traitement.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Format de sortie par défaut</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Qualité par défaut (1-100)</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Filtre de redimensionnement par défaut</label>
                <FormSelect 
                  value={settings.image_processing.filter}
                  onChange={(e) => {
                    const newSettings = { ...settings, image_processing: { ...settings.image_processing, filter: e.target.value } };
                    setSettings(newSettings);
                    handleUpdateSetting("image_processing", newSettings.image_processing);
                  }}
                >
                  <option value="lanczos3">Lanczos3 (Best Quality)</option>
                  <option value="triangle">Triangle (Faster)</option>
                  <option value="nearest">Nearest (Fastest)</option>
                </FormSelect>
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Max Allowed Width (px)</label>
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
            Cache
          </CardTitle>
          <CardDescription>Manage the image cache and storage</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="grid grid-cols-3 gap-4 p-4 bg-muted/30 rounded-lg">
              <div>
                <p className="text-sm text-muted-foreground">Cache Size</p>
                <p className="text-2xl font-semibold">{cacheStats.total_size_mb.toFixed(2)} MB</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Files</p>
                <p className="text-2xl font-semibold">{cacheStats.file_count}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Directory</p>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Cache Directory</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Max Size (MB)</label>
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
                  Clearing...
                </>
              ) : (
                <>
                  <Icon name="trash" size="sm" className="mr-2" />
                  Clear Cache
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
            Performance Limits
          </CardTitle>
          <CardDescription>Configure API performance, rate limiting, and thumbnail generation concurrency</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Concurrent Renders</label>
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
                  Maximum number of page renders and thumbnail generations running in parallel
                </p>
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Timeout (seconds)</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Rate Limit (req/s)</label>
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
              Note: Changes to limits require a server restart to take effect. The "Concurrent Renders" setting controls both page rendering and thumbnail generation parallelism.
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Thumbnail Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="image" size="md" />
            Thumbnails
          </CardTitle>
          <CardDescription>Configure thumbnail generation during indexing</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Enable Thumbnails</label>
                <FormSelect 
                  value={settings.thumbnail.enabled ? "true" : "false"}
                  onChange={(e) => {
                    const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, enabled: e.target.value === "true" } };
                    setSettings(newSettings);
                    handleUpdateSetting("thumbnail", newSettings.thumbnail);
                  }}
                >
                  <option value="true">Enabled</option>
                  <option value="false">Disabled</option>
                </FormSelect>
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Output Format</label>
                <FormSelect 
                  value={settings.thumbnail.format}
                  onChange={(e) => {
                    const newSettings = { ...settings, thumbnail: { ...settings.thumbnail, format: e.target.value } };
                    setSettings(newSettings);
                    handleUpdateSetting("thumbnail", newSettings.thumbnail);
                  }}
                >
                  <option value="original">Original (No Re-encoding)</option>
                  <option value="webp">WebP</option>
                  <option value="jpeg">JPEG</option>
                  <option value="png">PNG</option>
                </FormSelect>
                <p className="text-xs text-muted-foreground mt-1">
                  {settings.thumbnail.format === "original"
                    ? "Resizes to target dimensions, keeps source format (JPEG→JPEG). Much faster generation."
                    : "Resizes and re-encodes to selected format."}
                </p>
              </FormField>
            </FormRow>
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Width (px)</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Height (px)</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Quality (1-100)</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Thumbnail Directory</label>
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
                <p className="text-sm text-muted-foreground">Total Size</p>
                <p className="text-2xl font-semibold">{thumbnailStats.total_size_mb.toFixed(2)} MB</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Files</p>
                <p className="text-2xl font-semibold">{thumbnailStats.file_count}</p>
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Directory</p>
                <p className="text-sm font-mono truncate" title={thumbnailStats.directory}>{thumbnailStats.directory}</p>
              </div>
            </div>

            <p className="text-sm text-muted-foreground">
              Note: Thumbnail settings are used during indexing. Existing thumbnails will not be regenerated automatically. The concurrency for thumbnail generation is controlled by the "Concurrent Renders" setting in Performance Limits above.
            </p>
          </div>
        </CardContent>
      </Card>

      </>)}

      {activeTab === "integrations" && (<>
      {/* Metadata Providers */}
      <MetadataProvidersCard handleUpdateSetting={handleUpdateSetting} />

      {/* Komga Sync */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="refresh" size="md" />
            Komga Sync
          </CardTitle>
          <CardDescription>Import read status from a Komga server. Books are matched by title (case-insensitive). Credentials are not stored.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Komga URL</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Username</label>
                <FormInput
                  value={komgaUsername}
                  onChange={(e) => setKomgaUsername(e.target.value)}
                />
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Password</label>
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
                  Syncing...
                </>
              ) : (
                <>
                  <Icon name="refresh" size="sm" className="mr-2" />
                  Sync Read Books
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
                    <p className="text-sm text-muted-foreground">Komga read</p>
                    <p className="text-2xl font-semibold">{syncResult.total_komga_read}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Matched</p>
                    <p className="text-2xl font-semibold">{syncResult.matched}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Already read</p>
                    <p className="text-2xl font-semibold">{syncResult.already_read}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Newly marked</p>
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
                      {syncResult.matched_books.length} matched book{syncResult.matched_books.length !== 1 ? "s" : ""}
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
                      {syncResult.unmatched.length} unmatched book{syncResult.unmatched.length !== 1 ? "s" : ""}
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
                <h3 className="text-sm font-medium text-foreground mb-3">Sync History</h3>
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
                        <span>{r.total_komga_read} read</span>
                        <span>{r.matched} matched</span>
                        <span className="text-success">{r.newly_marked} new</span>
                        {r.unmatched_count > 0 && (
                          <span className="text-warning">{r.unmatched_count} unmatched</span>
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
                        <p className="text-sm text-muted-foreground">Komga read</p>
                        <p className="text-2xl font-semibold">{selectedReport.total_komga_read}</p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">Matched</p>
                        <p className="text-2xl font-semibold">{selectedReport.matched}</p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">Already read</p>
                        <p className="text-2xl font-semibold">{selectedReport.already_read}</p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">Newly marked</p>
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
                          {selectedReport.matched_books.length} matched book{selectedReport.matched_books.length !== 1 ? "s" : ""}
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
                          {selectedReport.unmatched.length} unmatched book{selectedReport.unmatched.length !== 1 ? "s" : ""}
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
          Metadata Providers
        </CardTitle>
        <CardDescription>Configure external metadata providers for series/book enrichment. Each library can override the default provider. All providers are available for quick-search in the metadata modal.</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-6">
          {/* Default provider */}
          <div>
            <label className="text-sm font-medium text-muted-foreground mb-2 block">Default Provider</label>
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
            <p className="text-xs text-muted-foreground mt-2">Used by default for metadata search. Libraries can override this individually.</p>
          </div>

          {/* Metadata language */}
          <div>
            <label className="text-sm font-medium text-muted-foreground mb-2 block">Metadata Language</label>
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
            <p className="text-xs text-muted-foreground mt-2">Preferred language for search results and descriptions. Fallback: English.</p>
          </div>

          {/* Provider API keys — always visible */}
          <div className="border-t border-border/50 pt-4">
            <h4 className="text-sm font-medium text-foreground mb-3">API Keys</h4>
            <div className="space-y-4">
              <FormField>
                <label className="text-sm font-medium text-muted-foreground mb-1 flex items-center gap-1.5">
                  <ProviderIcon provider="google_books" size={16} />
                  Google Books API Key
                </label>
                <FormInput
                  type="password"
                  placeholder="Optional — for higher rate limits"
                  value={apiKeys.google_books || ""}
                  onChange={(e) => setApiKeys({ ...apiKeys, google_books: e.target.value })}
                  onBlur={() => save(defaultProvider, metadataLanguage, apiKeys)}
                />
                <p className="text-xs text-muted-foreground mt-1">Works without a key but with lower rate limits.</p>
              </FormField>

              <FormField>
                <label className="text-sm font-medium text-muted-foreground mb-1 flex items-center gap-1.5">
                  <ProviderIcon provider="comicvine" size={16} />
                  ComicVine API Key
                </label>
                <FormInput
                  type="password"
                  placeholder="Required to use ComicVine"
                  value={apiKeys.comicvine || ""}
                  onChange={(e) => setApiKeys({ ...apiKeys, comicvine: e.target.value })}
                  onBlur={() => save(defaultProvider, metadataLanguage, apiKeys)}
                />
                <p className="text-xs text-muted-foreground mt-1">Get your key at <span className="font-mono text-foreground/70">comicvine.gamespot.com/api</span>.</p>
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
                <span className="text-xs text-muted-foreground">and</span>
                <div className="flex items-center gap-1.5">
                  <ProviderIcon provider="bedetheque" size={16} />
                  <span className="text-xs font-medium text-foreground">Bédéthèque</span>
                </div>
                <span className="text-xs text-muted-foreground">are free and require no API key.</span>
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

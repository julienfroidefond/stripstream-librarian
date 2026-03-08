"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, FormRow, Icon } from "../components/ui";
import { Settings, CacheStats, ClearCacheResponse, ThumbnailStats } from "../../lib/api";

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
        setSaveMessage("Settings saved successfully");
        setTimeout(() => setSaveMessage(null), 3000);
      } else {
        setSaveMessage("Failed to save settings");
      }
    } catch (error) {
      setSaveMessage("Error saving settings");
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
      setClearResult({ success: false, message: "Failed to clear cache" });
    } finally {
      setIsClearing(false);
    }
  }

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="settings" size="xl" />
          Settings
        </h1>
      </div>

      {saveMessage && (
        <Card className="mb-6 border-success/50 bg-success/5">
          <CardContent className="pt-6">
            <p className="text-success">{saveMessage}</p>
          </CardContent>
        </Card>
      )}

      {/* Image Processing Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="image" size="md" />
            Image Processing
          </CardTitle>
          <CardDescription>Configure how images are processed and compressed</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <FormRow>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Output Format</label>
                <FormSelect 
                  value={settings.image_processing.format}
                  onChange={(e) => {
                    const newSettings = { ...settings, image_processing: { ...settings.image_processing, format: e.target.value } };
                    setSettings(newSettings);
                    handleUpdateSetting("image_processing", newSettings.image_processing);
                  }}
                >
                  <option value="webp">WebP (Recommended)</option>
                  <option value="jpeg">JPEG</option>
                  <option value="png">PNG</option>
                </FormSelect>
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Quality (1-100)</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Resize Filter</label>
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
                <label className="text-sm font-medium text-muted-foreground mb-1 block">Max Width (px)</label>
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
          <CardDescription>Configure API performance and rate limiting</CardDescription>
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
              Note: Changes to limits require a server restart to take effect.
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
                  <option value="webp">WebP (Recommended)</option>
                  <option value="jpeg">JPEG</option>
                  <option value="png">PNG</option>
                </FormSelect>
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
              Note: Thumbnail settings are used during indexing. Existing thumbnails will not be regenerated automatically.
            </p>
          </div>
        </CardContent>
      </Card>
    </>
  );
}

"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, FormRow } from "../components/ui";
import { Settings, CacheStats, ClearCacheResponse } from "../../lib/api";

interface SettingsPageProps {
  initialSettings: Settings;
  initialCacheStats: CacheStats;
}

export default function SettingsPage({ initialSettings, initialCacheStats }: SettingsPageProps) {
  const [settings, setSettings] = useState<Settings>(initialSettings);
  const [cacheStats, setCacheStats] = useState<CacheStats>(initialCacheStats);
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
          <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
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
          <CardTitle>Image Processing</CardTitle>
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
          <CardTitle>Cache</CardTitle>
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
                  <svg className="animate-spin -ml-1 mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                  </svg>
                  Clearing...
                </>
              ) : (
                <>
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
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
          <CardTitle>Performance Limits</CardTitle>
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
    </>
  );
}

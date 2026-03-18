"use client";

import { useState, useRef, useEffect, useTransition } from "react";
import { Button } from "../components/ui";
import { ProviderIcon } from "../components/ProviderIcon";
import { useTranslation } from "../../lib/i18n/context";

interface LibraryActionsProps {
  libraryId: string;
  monitorEnabled: boolean;
  scanMode: string;
  watcherEnabled: boolean;
  metadataProvider: string | null;
  fallbackMetadataProvider: string | null;
  onUpdate?: () => void;
}

export function LibraryActions({
  libraryId,
  monitorEnabled,
  scanMode,
  watcherEnabled,
  metadataProvider,
  fallbackMetadataProvider,
  onUpdate
}: LibraryActionsProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [isPending, startTransition] = useTransition();
  const [saveError, setSaveError] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleSubmit = (formData: FormData) => {
    setSaveError(null);
    startTransition(async () => {
      const monitorEnabled = formData.get("monitor_enabled") === "true";
      const watcherEnabled = formData.get("watcher_enabled") === "true";
      const scanMode = formData.get("scan_mode") as string;
      const newMetadataProvider = (formData.get("metadata_provider") as string) || null;
      const newFallbackProvider = (formData.get("fallback_metadata_provider") as string) || null;

      try {
        const [response] = await Promise.all([
          fetch(`/api/libraries/${libraryId}/monitoring`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              monitor_enabled: monitorEnabled,
              scan_mode: scanMode,
              watcher_enabled: watcherEnabled,
            }),
          }),
          fetch(`/api/libraries/${libraryId}/metadata-provider`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ metadata_provider: newMetadataProvider, fallback_metadata_provider: newFallbackProvider }),
          }),
        ]);

        if (response.ok) {
          setIsOpen(false);
          window.location.reload();
        } else {
          const body = await response.json().catch(() => ({}));
          const msg = body?.error || `HTTP ${response.status}`;
          console.error("Failed to save settings:", msg);
          setSaveError(msg);
        }
      } catch (error) {
        const msg = error instanceof Error ? error.message : "Network error";
        console.error("Failed to save settings:", msg);
        setSaveError(msg);
      }
    });
  };

  return (
    <div className="relative" ref={dropdownRef}>
      <Button 
        variant="ghost" 
        size="sm"
        onClick={() => setIsOpen(!isOpen)}
        className={isOpen ? "bg-accent" : ""}
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
      </Button>

      {isOpen && (
        <div className="absolute right-0 top-full mt-2 w-72 bg-card rounded-xl shadow-md border border-border/60 p-4 z-50">
          <form action={handleSubmit}>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground flex items-center gap-2">
                  <input 
                    type="checkbox" 
                    name="monitor_enabled" 
                    value="true"
                    defaultChecked={monitorEnabled}
                    className="w-4 h-4 rounded border-border text-primary focus:ring-ring"
                  />
                  {t("libraryActions.autoScan")}
                </label>
              </div>

              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground flex items-center gap-2">
                  <input 
                    type="checkbox" 
                    name="watcher_enabled" 
                    value="true"
                    defaultChecked={watcherEnabled}
                    className="w-4 h-4 rounded border-border text-primary focus:ring-ring"
                  />
                  {t("libraryActions.fileWatch")}
                </label>
              </div>

              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground">{t("libraryActions.schedule")}</label>
                <select 
                  name="scan_mode" 
                  defaultValue={scanMode}
                  className="text-sm border border-border rounded-lg px-2 py-1 bg-background"
                >
                  <option value="manual">{t("monitoring.manual")}</option>
                  <option value="hourly">{t("monitoring.hourly")}</option>
                  <option value="daily">{t("monitoring.daily")}</option>
                  <option value="weekly">{t("monitoring.weekly")}</option>
                </select>
              </div>

              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground flex items-center gap-1.5">
                  {metadataProvider && <ProviderIcon provider={metadataProvider} size={16} />}
                  {t("libraryActions.provider")}
                </label>
                <select
                  name="metadata_provider"
                  defaultValue={metadataProvider || ""}
                  className="text-sm border border-border rounded-lg px-2 py-1 bg-background"
                >
                  <option value="">{t("libraryActions.default")}</option>
                  <option value="none">{t("libraryActions.none")}</option>
                  <option value="google_books">Google Books</option>
                  <option value="comicvine">ComicVine</option>
                  <option value="open_library">Open Library</option>
                  <option value="anilist">AniList</option>
                  <option value="bedetheque">Bédéthèque</option>
                </select>
              </div>

              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground flex items-center gap-1.5">
                  {fallbackMetadataProvider && fallbackMetadataProvider !== "none" && <ProviderIcon provider={fallbackMetadataProvider} size={16} />}
                  {t("libraryActions.fallback")}
                </label>
                <select
                  name="fallback_metadata_provider"
                  defaultValue={fallbackMetadataProvider || ""}
                  className="text-sm border border-border rounded-lg px-2 py-1 bg-background"
                >
                  <option value="">{t("libraryActions.none")}</option>
                  <option value="google_books">Google Books</option>
                  <option value="comicvine">ComicVine</option>
                  <option value="open_library">Open Library</option>
                  <option value="anilist">AniList</option>
                  <option value="bedetheque">Bédéthèque</option>
                </select>
              </div>

              {saveError && (
                <p className="text-xs text-destructive bg-destructive/10 px-2 py-1.5 rounded-lg break-all">
                  {saveError}
                </p>
              )}

              <Button
                type="submit"
                size="sm"
                className="w-full"
                disabled={isPending}
              >
                {isPending ? t("libraryActions.saving") : t("common.save")}
              </Button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}

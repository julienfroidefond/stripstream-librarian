"use client";

import { useState, useTransition } from "react";
import { createPortal } from "react-dom";
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
  metadataRefreshMode: string;
  onUpdate?: () => void;
}

export function LibraryActions({
  libraryId,
  monitorEnabled,
  scanMode,
  watcherEnabled,
  metadataProvider,
  fallbackMetadataProvider,
  metadataRefreshMode,
}: LibraryActionsProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [isPending, startTransition] = useTransition();
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleSubmit = (formData: FormData) => {
    setSaveError(null);
    startTransition(async () => {
      const monitorEnabled = formData.get("monitor_enabled") === "true";
      const watcherEnabled = formData.get("watcher_enabled") === "true";
      const scanMode = formData.get("scan_mode") as string;
      const newMetadataProvider = (formData.get("metadata_provider") as string) || null;
      const newFallbackProvider = (formData.get("fallback_metadata_provider") as string) || null;
      const newMetadataRefreshMode = formData.get("metadata_refresh_mode") as string;

      try {
        const [response] = await Promise.all([
          fetch(`/api/libraries/${libraryId}/monitoring`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              monitor_enabled: monitorEnabled,
              scan_mode: scanMode,
              watcher_enabled: watcherEnabled,
              metadata_refresh_mode: newMetadataRefreshMode,
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
    <>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setIsOpen(true)}
        className={isOpen ? "bg-accent" : ""}
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
      </Button>

      {isOpen && createPortal(
        <>
          {/* Backdrop */}
          <div
            className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50"
            onClick={() => setIsOpen(false)}
          />

          {/* Modal */}
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-200">
              {/* Header */}
              <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30">
                <div className="flex items-center gap-2.5">
                  <svg className="w-5 h-5 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                  </svg>
                  <span className="font-semibold text-lg">{t("libraryActions.settingsTitle")}</span>
                </div>
                <button
                  type="button"
                  onClick={() => setIsOpen(false)}
                  className="text-muted-foreground hover:text-foreground transition-colors p-1.5 hover:bg-accent rounded-lg"
                >
                  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              {/* Form */}
              <form action={handleSubmit}>
                <div className="p-6 space-y-8 max-h-[70vh] overflow-y-auto">

                  {/* Section: Indexation */}
                  <div className="space-y-5">
                    <h3 className="flex items-center gap-2 text-sm font-semibold text-foreground uppercase tracking-wide">
                      <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                      </svg>
                      {t("libraryActions.sectionIndexation")}
                    </h3>

                    {/* Auto scan */}
                    <div className="flex items-start justify-between gap-4">
                      <div className="flex-1">
                        <label className="text-sm font-medium text-foreground flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            name="monitor_enabled"
                            value="true"
                            defaultChecked={monitorEnabled}
                            className="w-4 h-4 rounded border-border text-primary focus:ring-ring"
                          />
                          {t("libraryActions.autoScan")}
                        </label>
                        <p className="text-xs text-muted-foreground mt-1.5 ml-6">{t("libraryActions.autoScanDesc")}</p>
                      </div>
                      <select
                        name="scan_mode"
                        defaultValue={scanMode}
                        className="text-sm border border-border rounded-lg px-3 py-1.5 bg-background min-w-[130px] shrink-0"
                      >
                        <option value="manual">{t("monitoring.manual")}</option>
                        <option value="hourly">{t("monitoring.hourly")}</option>
                        <option value="daily">{t("monitoring.daily")}</option>
                        <option value="weekly">{t("monitoring.weekly")}</option>
                      </select>
                    </div>

                    {/* File watcher */}
                    <div>
                      <label className="text-sm font-medium text-foreground flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          name="watcher_enabled"
                          value="true"
                          defaultChecked={watcherEnabled}
                          className="w-4 h-4 rounded border-border text-primary focus:ring-ring"
                        />
                        {t("libraryActions.fileWatch")}
                      </label>
                      <p className="text-xs text-muted-foreground mt-1.5 ml-6">{t("libraryActions.fileWatchDesc")}</p>
                    </div>
                  </div>

                  <hr className="border-border/40" />

                  {/* Section: Metadata */}
                  <div className="space-y-5">
                    <h3 className="flex items-center gap-2 text-sm font-semibold text-foreground uppercase tracking-wide">
                      <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
                      </svg>
                      {t("libraryActions.sectionMetadata")}
                    </h3>

                    {/* Provider */}
                    <div>
                      <div className="flex items-center justify-between gap-4">
                        <label className="text-sm font-medium text-foreground flex items-center gap-1.5">
                          {metadataProvider && metadataProvider !== "none" && <ProviderIcon provider={metadataProvider} size={16} />}
                          {t("libraryActions.provider")}
                        </label>
                        <select
                          name="metadata_provider"
                          defaultValue={metadataProvider || ""}
                          className="text-sm border border-border rounded-lg px-3 py-1.5 bg-background min-w-[160px] shrink-0"
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
                      <p className="text-xs text-muted-foreground mt-1.5">{t("libraryActions.providerDesc")}</p>
                    </div>

                    {/* Fallback */}
                    <div>
                      <div className="flex items-center justify-between gap-4">
                        <label className="text-sm font-medium text-foreground flex items-center gap-1.5">
                          {fallbackMetadataProvider && fallbackMetadataProvider !== "none" && <ProviderIcon provider={fallbackMetadataProvider} size={16} />}
                          {t("libraryActions.fallback")}
                        </label>
                        <select
                          name="fallback_metadata_provider"
                          defaultValue={fallbackMetadataProvider || ""}
                          className="text-sm border border-border rounded-lg px-3 py-1.5 bg-background min-w-[160px] shrink-0"
                        >
                          <option value="">{t("libraryActions.none")}</option>
                          <option value="google_books">Google Books</option>
                          <option value="comicvine">ComicVine</option>
                          <option value="open_library">Open Library</option>
                          <option value="anilist">AniList</option>
                          <option value="bedetheque">Bédéthèque</option>
                        </select>
                      </div>
                      <p className="text-xs text-muted-foreground mt-1.5">{t("libraryActions.fallbackDesc")}</p>
                    </div>

                    {/* Metadata refresh */}
                    <div>
                      <div className="flex items-center justify-between gap-4">
                        <label className="text-sm font-medium text-foreground">{t("libraryActions.metadataRefreshSchedule")}</label>
                        <select
                          name="metadata_refresh_mode"
                          defaultValue={metadataRefreshMode}
                          className="text-sm border border-border rounded-lg px-3 py-1.5 bg-background min-w-[160px] shrink-0"
                        >
                          <option value="manual">{t("monitoring.manual")}</option>
                          <option value="hourly">{t("monitoring.hourly")}</option>
                          <option value="daily">{t("monitoring.daily")}</option>
                          <option value="weekly">{t("monitoring.weekly")}</option>
                        </select>
                      </div>
                      <p className="text-xs text-muted-foreground mt-1.5">{t("libraryActions.metadataRefreshDesc")}</p>
                    </div>
                  </div>

                  {saveError && (
                    <p className="text-sm text-destructive bg-destructive/10 px-3 py-2 rounded-lg break-all">
                      {saveError}
                    </p>
                  )}
                </div>

                {/* Footer */}
                <div className="flex items-center justify-end gap-2 px-5 py-4 border-t border-border/50 bg-muted/30">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setIsOpen(false)}
                  >
                    {t("common.cancel")}
                  </Button>
                  <Button
                    type="submit"
                    size="sm"
                    disabled={isPending}
                  >
                    {isPending ? t("libraryActions.saving") : t("common.save")}
                  </Button>
                </div>
              </form>
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  );
}

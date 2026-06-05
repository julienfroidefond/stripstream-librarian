"use client";

import { useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./ui";
import { useTranslation } from "../../lib/i18n/context";
import { stripLeadingArticle } from "../../lib/volumeRanges";
import type { TelegramSearchResultDto } from "../../lib/api";
import type { TranslationKey } from "../../lib/i18n/fr";

type TFunction = (key: TranslationKey, vars?: Record<string, string | number>) => string;

function tgStatusClass(status: string): string {
  switch (status) {
    case "available":    return "bg-sky-500/15 text-sky-600";
    case "downloading":  return "bg-primary/10 text-primary";
    case "imported":     return "bg-success/10 text-success";
    case "failed":       return "bg-destructive/10 text-destructive";
    default:             return "bg-muted/30 text-muted-foreground";
  }
}

function tgStatusLabel(status: string, t: TFunction): string {
  const map: Record<string, TranslationKey> = {
    available:   "telegramMonitor.statusAvailable",
    downloading: "downloads.status.downloading",
    imported:    "downloads.status.imported",
    failed:      "downloads.status.error",
  };
  return t(map[status] ?? status);
}

interface MissingBookItem {
  title: string | null;
  volume_number: number | null;
}

interface TelegramSearchModalProps {
  seriesName: string;
  missingBooks?: MissingBookItem[] | null;
  ownedVolumes?: number[];
  initialEnabled?: boolean;
  children?: (open: () => void) => React.ReactNode;
}

function formatSize(bytes: number | null): string {
  if (!bytes) return "";
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(0) + " KB";
  return bytes + " B";
}

export function TelegramSearchModal({ seriesName, missingBooks, ownedVolumes, initialEnabled, children }: TelegramSearchModalProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [searchInput, setSearchInput] = useState(seriesName);
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<TelegramSearchResultDto[]>([]);
  const [searched, setSearched] = useState(false);
  const [downloadingIds, setDownloadingIds] = useState<Set<string>>(new Set());
  const [dismissingIds, setDismissingIds] = useState<Set<string>>(new Set());

  const ownedVolumeSet = new Set(ownedVolumes ?? []);

  const doSearch = useCallback(async (query: string) => {
    if (!query.trim()) return;
    setIsSearching(true);
    setSearched(false);
    try {
      const resp = await fetch("/api/telegram-monitor/live-search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query: query.trim() }),
      });
      if (resp.ok) setResults(await resp.json());
      setSearched(true);
    } finally {
      setIsSearching(false);
    }
  }, []);

  const strippedName = stripLeadingArticle(seriesName);

  function handleOpen() {
    setIsOpen(true);
    setSearchInput(strippedName);
    setResults([]);
    setSearched(false);
    doSearch(strippedName);
  }

  function handleClose() {
    setIsOpen(false);
  }

  async function handleDownload(bookId: string) {
    setDownloadingIds(prev => new Set(prev).add(bookId));
    try {
      const resp = await fetch(`/api/telegram-monitor/books/${bookId}/download`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      if (resp.ok) setResults(prev => prev.filter(b => b.id !== bookId));
    } finally {
      setDownloadingIds(prev => { const s = new Set(prev); s.delete(bookId); return s; });
    }
  }

  async function handleDismiss(bookId: string) {
    setDismissingIds(prev => new Set(prev).add(bookId));
    try {
      const resp = await fetch(`/api/telegram-monitor/books/${bookId}`, { method: "DELETE" });
      if (resp.ok) setResults(prev => prev.filter(b => b.id !== bookId));
    } finally {
      setDismissingIds(prev => { const s = new Set(prev); s.delete(bookId); return s; });
    }
  }

  if (initialEnabled === false) return null;

  const modal = isOpen ? createPortal(
    <>
      <div className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50" onClick={handleClose} />
      <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
        <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-2xl max-h-[85vh] flex flex-col animate-in fade-in zoom-in-95 duration-200">

          {/* Header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30 rounded-t-xl sticky top-0 z-10">
            <div className="flex items-center gap-2">
              <Icon name="send" size="sm" className="text-sky-500" />
              <h3 className="font-semibold text-foreground">{t("telegramMonitor.searchModalTitle", { series: seriesName })}</h3>
            </div>
            <button type="button" onClick={handleClose} className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors">
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
          </div>

          {/* Body */}
          <div className="flex-1 overflow-y-auto p-5 space-y-4">
            {/* Search input */}
            <form
              onSubmit={(e) => { e.preventDefault(); doSearch(searchInput); }}
              className="flex items-center gap-2"
            >
              <input
                type="text"
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                className="flex-1 px-3 py-2 rounded-lg border border-border bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
                placeholder={t("telegramMonitor.searchPlaceholder")}
              />
              <button
                type="submit"
                disabled={isSearching || !searchInput.trim()}
                className="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
              >
                <Icon name="search" size="sm" />
                {t("prowlarr.searchAction")}
              </button>
            </form>

            {/* Quick badges: series name (original + stripped if different) + missing volumes */}
            <div className="flex flex-wrap items-center gap-2 max-h-24 overflow-y-auto">
              <button
                type="button"
                onClick={() => { setSearchInput(strippedName); doSearch(strippedName); }}
                disabled={isSearching}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors border-primary/50 bg-primary/10 text-primary hover:bg-primary/20"
              >
                {strippedName}
              </button>
              {strippedName !== seriesName && (
                <button
                  type="button"
                  onClick={() => { setSearchInput(seriesName); doSearch(seriesName); }}
                  disabled={isSearching}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors border-primary/30 bg-primary/5 text-primary/70 hover:bg-primary/15"
                >
                  {seriesName}
                </button>
              )}
              {(missingBooks ?? []).map((book, i) => {
                const label = book.title || (book.volume_number != null ? `T${book.volume_number}` : null);
                if (!label) return null;
                const q = book.volume_number != null ? `${strippedName} T${book.volume_number}` : `${strippedName} ${label}`;
                return (
                  <button
                    key={i}
                    type="button"
                    onClick={() => { setSearchInput(q); doSearch(q); }}
                    disabled={isSearching}
                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors border-green-500/50 bg-green-500/10 text-green-600 hover:bg-green-500/20"
                  >
                    {label}
                  </button>
                );
              })}
            </div>

            {/* Searching */}
            {isSearching && (
              <div className="flex items-center gap-2 text-muted-foreground text-sm">
                <Icon name="spinner" size="sm" className="animate-spin" />
                {t("telegramMonitor.liveSearching")}
              </div>
            )}

            {/* Results */}
            {!isSearching && results.length > 0 && (
              <div>
                <p className="text-xs text-muted-foreground mb-3">
                  {t("telegramMonitor.availableCount", { count: results.length, plural: results.length > 1 ? "s" : "" })}
                </p>
                <div className="rounded-lg border border-border overflow-hidden">
                  {results.map(book => {
                    const isMissing = book.volume_number != null && ownedVolumes != null && !ownedVolumeSet.has(book.volume_number);
                    return (
                    <div
                      key={book.id}
                      className={`flex items-center gap-2 sm:gap-3 px-3 py-2.5 border-b border-border/40 last:border-b-0 transition-colors ${isMissing ? "bg-green-500/10 hover:bg-green-500/20 border-l-2 border-l-green-500" : "hover:bg-muted/20"}`}
                    >
                      {book.volume_number != null ? (
                        <span className={`px-1.5 py-0.5 rounded text-xs font-medium shrink-0 tabular-nums ${isMissing ? "bg-green-500/20 text-green-600" : "bg-success/20 text-success"}`}>
                          T{String(book.volume_number).padStart(2, "0")}
                        </span>
                      ) : (
                        <span className="px-1.5 py-0.5 rounded bg-muted/50 text-muted-foreground text-xs shrink-0">—</span>
                      )}

                      <div className="flex-1 min-w-0">
                        <p className="text-sm text-foreground truncate" title={book.filename}>{book.filename}</p>
                        <div className="flex items-center gap-2 mt-0.5 text-[11px] text-muted-foreground">
                          <span>@{book.channel_username}</span>
                          {book.file_size && <span>{formatSize(book.file_size)}</span>}
                          {book.series_name && book.series_name !== seriesName && (
                            <span className="px-1 py-px rounded bg-warning/10 text-warning text-[10px]">{book.series_name}</span>
                          )}
                        </div>
                      </div>

                      <div className="flex items-center gap-2 shrink-0">
                        <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full ${tgStatusClass(book.status)}`}>
                          {tgStatusLabel(book.status, t)}
                        </span>
                        {book.status === "available" && (
                          <>
                            <button
                              type="button"
                              onClick={() => handleDownload(book.id)}
                              disabled={downloadingIds.has(book.id)}
                              title={t("telegramMonitor.download")}
                              className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-success hover:bg-success/10 transition-colors disabled:opacity-30"
                            >
                              {downloadingIds.has(book.id)
                                ? <Icon name="spinner" size="sm" className="animate-spin" />
                                : <Icon name="download" size="sm" />}
                            </button>
                            <button
                              type="button"
                              onClick={() => handleDismiss(book.id)}
                              disabled={dismissingIds.has(book.id)}
                              title={t("telegramMonitor.dismiss")}
                              className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30"
                            >
                              {dismissingIds.has(book.id)
                                ? <Icon name="spinner" size="sm" className="animate-spin" />
                                : <Icon name="trash" size="sm" />}
                            </button>
                          </>
                        )}
                        {book.status === "downloading" && (
                          <Icon name="spinner" size="sm" className="animate-spin text-primary" />
                        )}
                        {book.status === "imported" && (
                          <Icon name="check" size="sm" className="text-success" />
                        )}
                      </div>
                    </div>
                  ); })}
                </div>
              </div>
            )}

            {/* No results */}
            {!isSearching && searched && results.length === 0 && (
              <div className="flex flex-col items-center justify-center gap-3 py-8 text-muted-foreground">
                <Icon name="send" size="xl" className="opacity-20 text-sky-500" />
                <p className="text-sm">{t("telegramMonitor.noAvailableForSeries")}</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </>,
    document.body,
  ) : null;

  return (
    <>
      {children ? (
        children(handleOpen)
      ) : (
        <button
          type="button"
          onClick={handleOpen}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium border border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
        >
          <Icon name="send" size="sm" className="text-sky-500" />
          {t("telegramMonitor.searchButton")}
        </button>
      )}
      {modal}
    </>
  );
}

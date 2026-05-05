"use client";

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import { TorrentDownloadDto, LatestFoundPerLibraryDto } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle, Button, Icon } from "@/app/components/ui";
import { QbittorrentProvider, QbittorrentDownloadButton } from "@/app/components/QbittorrentDownloadButton";
import { useTranslation } from "@/lib/i18n/context";
import { compressVolumes } from "@/lib/volumeRanges";
import type { TranslationKey } from "@/lib/i18n/fr";

type TFunction = (key: TranslationKey, vars?: Record<string, string | number>) => string;

function formatRelativeDate(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "à l'instant";
  if (diffMin < 60) return `il y a ${diffMin}min`;
  const diffH = Math.floor(diffMin / 60);
  if (diffH < 24) return `il y a ${diffH}h`;
  const diffD = Math.floor(diffH / 24);
  if (diffD < 30) return `il y a ${diffD}j`;
  return date.toLocaleDateString();
}

const STATUS_ACTIVE = new Set(["downloading", "completed", "importing"]);

/** Group releases by identical title, preserving order of first occurrence. */
function groupReleasesByTitle<T extends { title: string }>(releases: T[]): { title: string; items: T[]; originalIndices: number[] }[] {
  const groups: Map<string, { items: T[]; originalIndices: number[] }> = new Map();
  releases.forEach((r, idx) => {
    const existing = groups.get(r.title);
    if (existing) {
      existing.items.push(r);
      existing.originalIndices.push(idx);
    } else {
      groups.set(r.title, { items: [r], originalIndices: [idx] });
    }
  });
  return Array.from(groups.entries()).map(([title, { items, originalIndices }]) => ({ title, items, originalIndices }));
}

function statusLabel(status: string, t: TFunction): string {
  const map: Record<string, TranslationKey> = {
    downloading:       "downloads.status.downloading",
    completed:         "downloads.status.completed",
    importing:         "downloads.status.importing",
    imported:          "downloads.status.imported",
    partial:           "downloads.status.partial",
    no_files_imported: "downloads.status.noFilesImported",
    error:             "downloads.status.error",
  };
  return t(map[status] ?? status);
}

function statusClass(status: string): string {
  switch (status) {
    case "downloading": return "bg-primary/10 text-primary";
    case "completed":   return "bg-warning/10 text-warning";
    case "importing":   return "bg-primary/10 text-primary";
    case "imported":    return "bg-success/10 text-success";
    case "partial":     return "bg-warning/10 text-warning";
    case "no_files_imported": return "bg-destructive/10 text-destructive";
    case "error":       return "bg-destructive/10 text-destructive";
    default:            return "bg-muted/30 text-muted-foreground";
  }
}

function formatVolumes(vols: number[]): string {
  return [...vols].sort((a, b) => a - b).map(v => `T${String(v).padStart(2, "0")}`).join(", ");
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString("fr-FR", {
    day: "2-digit", month: "2-digit", year: "numeric",
    hour: "2-digit", minute: "2-digit",
  });
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
}

function formatEta(seconds: number): string {
  if (seconds <= 0 || seconds >= 8640000) return "";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}h${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m${String(s).padStart(2, "0")}s`;
  return `${s}s`;
}

interface DownloadsPageProps {
  initialDownloads: TorrentDownloadDto[];
  initialLatestFound: LatestFoundPerLibraryDto[];
  qbConfigured?: boolean;
}

const PAGE_SIZE = 10;

export function DownloadsPage({ initialDownloads, initialLatestFound, qbConfigured }: DownloadsPageProps) {
  const { t } = useTranslation();
  const [downloads, setDownloads] = useState<TorrentDownloadDto[]>(initialDownloads);
  const [latestFound, setLatestFound] = useState<LatestFoundPerLibraryDto[]>(initialLatestFound);
  const [filter, setFilter] = useState<string>("all");
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [page, setPage] = useState(1);

  const refresh = useCallback(async (showSpinner = true) => {
    if (showSpinner) setIsRefreshing(true);
    try {
      const [dlResp, lfResp] = await Promise.all([
        fetch("/api/torrent-downloads"),
        fetch("/api/download-detection/latest-found"),
      ]);
      if (dlResp.ok) setDownloads(await dlResp.json());
      if (lfResp.ok) setLatestFound(await lfResp.json());
    } finally {
      if (showSpinner) setIsRefreshing(false);
    }
  }, []);

  // Auto-refresh every 5s while there are active downloads
  const hasActive = downloads.some(d => STATUS_ACTIVE.has(d.status));
  useEffect(() => {
    if (!hasActive) return;
    const id = setInterval(() => refresh(false), 2000);
    return () => clearInterval(id);
  }, [hasActive, refresh]);

  const filters = [
    { id: "all",    label: t("common.all") },
    { id: "active", label: t("downloads.filterActive") },
    { id: "imported", label: t("downloads.status.imported") },
    { id: "error",  label: t("downloads.status.error") },
  ];

  const visible = downloads.filter(d => {
    if (filter === "all") return true;
    if (filter === "active") return STATUS_ACTIVE.has(d.status);
    return d.status === filter;
  });

  const totalPages = Math.ceil(visible.length / PAGE_SIZE);
  const paged = visible.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  // Reset to page 1 when filter changes
  const handleFilterChange = (id: string) => { setFilter(id); setPage(1); };

  return (
    <>
      <div className="flex items-center justify-between mb-4 sm:mb-6 gap-2">
        <h1 className="text-xl sm:text-3xl font-bold text-foreground flex items-center gap-2 sm:gap-3">
          <Icon name="download" size="lg" className="sm:hidden text-emerald-500" />
          <Icon name="download" size="xl" className="hidden sm:block text-emerald-500" />
          {t("downloads.title")}
        </h1>
        <Button onClick={() => refresh(true)} disabled={isRefreshing} variant="outline" size="sm">
          {isRefreshing ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="refresh" size="sm" />}
          <span className="ml-2 hidden sm:inline">{t("downloads.refresh")}</span>
        </Button>
      </div>

      {/* Filter bar */}
      <div className="flex gap-1 mb-4 border-b border-border overflow-x-auto scrollbar-none">
        {filters.map(f => (
          <button
            key={f.id}
            onClick={() => handleFilterChange(f.id)}
            className={`px-3 sm:px-4 py-2 text-xs sm:text-sm font-medium border-b-2 transition-colors -mb-px whitespace-nowrap ${
              filter === f.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
            }`}
          >
            {f.label}
            {f.id !== "all" && (
              <span className="ml-1 sm:ml-1.5 text-[10px] sm:text-xs opacity-60">
                {downloads.filter(d => f.id === "active" ? STATUS_ACTIVE.has(d.status) : d.status === f.id).length}
              </span>
            )}
          </button>
        ))}
      </div>

      {visible.length === 0 ? (
        <Card className="mt-4">
          <CardContent className="pt-16 pb-16 flex flex-col items-center justify-center gap-3 text-muted-foreground">
            <Icon name="download" size="xl" className="opacity-30" />
            <p className="text-sm">{t("downloads.empty")}</p>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="space-y-1.5">
            {paged.map(dl => (
              <DownloadRow key={dl.id} dl={dl} onDeleted={() => refresh(false)} onRetried={() => refresh(false)} />
            ))}
          </div>
          {totalPages > 1 && (
            <div className="flex items-center justify-center gap-2 mt-4">
              <Button variant="outline" size="xs" disabled={page <= 1} onClick={() => setPage(p => p - 1)}>
                <Icon name="chevronLeft" size="sm" />
              </Button>
              <span className="text-xs text-muted-foreground tabular-nums">
                {page} / {totalPages}
              </span>
              <Button variant="outline" size="xs" disabled={page >= totalPages} onClick={() => setPage(p => p + 1)}>
                <Icon name="chevronRight" size="sm" />
              </Button>
            </div>
          )}
        </>
      )}

      {/* Available downloads from latest detection */}
      {latestFound.length > 0 && (
        <QbittorrentProvider initialConfigured={qbConfigured} onDownloadStarted={() => refresh(false)}>
          <AvailableDownloadsSection latestFound={latestFound} onDeleted={() => refresh(false)} />
        </QbittorrentProvider>
      )}
    </>
  );
}

const RETRYABLE_STATUSES = new Set(["importing", "error", "no_files_imported", "partial"]);

function DownloadRow({ dl, onDeleted, onRetried }: { dl: TorrentDownloadDto; onDeleted: () => void; onRetried: () => void }) {
  const { t } = useTranslation();
  const [deleting, setDeleting] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const allImported = Array.isArray(dl.imported_files) ? dl.imported_files : [];
  const importedCount = allImported.filter((f: { already_existed?: boolean }) => !f.already_existed).length;
  const alreadyExistedCount = allImported.filter((f: { already_existed?: boolean }) => f.already_existed).length;

  async function handleDelete() {
    setDeleting(true);
    setShowConfirm(false);
    try {
      const resp = await fetch(`/api/torrent-downloads/${dl.id}`, { method: "DELETE" });
      if (resp.ok) onDeleted();
    } finally {
      setDeleting(false);
    }
  }

  async function handleRetry() {
    setRetrying(true);
    try {
      const resp = await fetch(`/api/torrent-downloads/${dl.id}/retry`, { method: "POST" });
      if (resp.ok) onRetried();
    } finally {
      setRetrying(false);
    }
  }

  const canRetry = RETRYABLE_STATUSES.has(dl.status);

  const statusIcon = dl.status === "importing" ? (
    <Icon name="spinner" size="sm" className="animate-spin text-primary" />
  ) : dl.status === "imported" ? (
    <Icon name="check" size="sm" className="text-success" />
  ) : dl.status === "error" || dl.status === "no_files_imported" ? (
    <Icon name="warning" size="sm" className="text-destructive" />
  ) : dl.status === "partial" ? (
    <Icon name="warning" size="sm" className="text-warning" />
  ) : dl.status === "downloading" ? (
    <Icon name="download" size="sm" className="text-primary" />
  ) : (
    <Icon name="refresh" size="sm" className="text-warning" />
  );

  return (
    <>
      <div className="flex items-start sm:items-center gap-2 sm:gap-3 px-3 py-2 rounded-lg border border-border/40 bg-card hover:bg-accent/30 transition-colors">
        <div className="mt-0.5 sm:mt-0">{statusIcon}</div>

        <div className="flex-1 min-w-0">
          {/* Desktop: single row */}
          <div className="hidden sm:flex items-center gap-2">
            <Link href={`/series/${dl.series_id ?? encodeURIComponent(dl.series_name)}`} className="text-sm font-medium text-primary hover:underline truncate">{dl.series_name}</Link>
            <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full ${statusClass(dl.status)}`}>
              {statusLabel(dl.status, t)}
            </span>
            {dl.expected_volumes.length > 0 && (
              <span className="text-[11px] text-muted-foreground">{formatVolumes(dl.expected_volumes)}</span>
            )}
            {dl.status === "imported" && importedCount > 0 && (
              <span className="text-[11px] text-success">{importedCount} {t("downloads.filesImported")}</span>
            )}
            {dl.status === "imported" && importedCount === 0 && alreadyExistedCount > 0 && (
              <span className="text-[11px] text-muted-foreground">{alreadyExistedCount} {t("downloads.alreadyExisted")}</span>
            )}
            {dl.status === "imported" && importedCount > 0 && alreadyExistedCount > 0 && (
              <span className="text-[11px] text-muted-foreground">({alreadyExistedCount} {t("downloads.alreadyExisted")})</span>
            )}
          </div>

          {/* Mobile: stacked */}
          <div className="sm:hidden">
            <div className="flex items-center gap-1.5">
              <Link href={`/series/${dl.series_id ?? encodeURIComponent(dl.series_name)}`} className="text-sm font-medium text-primary hover:underline truncate">{dl.series_name}</Link>
              <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full shrink-0 ${statusClass(dl.status)}`}>
                {statusLabel(dl.status, t)}
              </span>
            </div>
            <div className="flex items-center gap-2 mt-0.5 text-[11px] text-muted-foreground">
              {dl.expected_volumes.length > 0 && <span>{formatVolumes(dl.expected_volumes)}</span>}
              {dl.status === "imported" && importedCount > 0 && (
                <span className="text-success">{importedCount} {t("downloads.filesImported")}</span>
              )}
              {dl.status === "imported" && importedCount === 0 && alreadyExistedCount > 0 && (
                <span>{alreadyExistedCount} {t("downloads.alreadyExisted")}</span>
              )}
              <span className="tabular-nums">{formatDate(dl.created_at)}</span>
            </div>
          </div>

          {dl.status === "downloading" && (
            <div className="flex items-center gap-2 mt-1">
              <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden max-w-xs">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-500"
                  style={{ width: `${Math.round(dl.progress * 100)}%` }}
                />
              </div>
              <span className="text-[10px] font-medium text-foreground tabular-nums">
                {Math.round(dl.progress * 100)}%
              </span>
              {dl.download_speed > 0 && (
                <span className="text-[10px] text-muted-foreground hidden sm:inline">{formatSpeed(dl.download_speed)}</span>
              )}
              {dl.eta > 0 && dl.eta < 8640000 && (
                <span className="text-[10px] text-muted-foreground">ETA {formatEta(dl.eta)}</span>
              )}
            </div>
          )}

          {dl.error_message && (
            <p className="text-[11px] text-destructive truncate mt-0.5" title={dl.error_message}>{dl.error_message}</p>
          )}
        </div>

        <span className="text-[10px] text-muted-foreground shrink-0 tabular-nums hidden sm:block">{formatDate(dl.created_at)}</span>

        {canRetry && (
          <button
            type="button"
            onClick={handleRetry}
            disabled={retrying}
            className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors disabled:opacity-30 shrink-0"
            title={t("downloads.retry")}
          >
            {retrying ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="refresh" size="sm" />}
          </button>
        )}
        <button
          type="button"
          onClick={() => setShowConfirm(true)}
          disabled={deleting || dl.status === "importing"}
          className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
          title={dl.status === "downloading" ? t("downloads.cancel") : t("downloads.delete")}
        >
          {deleting ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="trash" size="sm" />}
        </button>
      </div>

      {showConfirm && createPortal(
        <>
          <div className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50" onClick={() => setShowConfirm(false)} />
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-sm overflow-hidden animate-in fade-in zoom-in-95 duration-200">
              <div className="p-6">
                <h3 className="text-lg font-semibold text-foreground mb-2">
                  {dl.status === "downloading" ? t("downloads.cancel") : t("downloads.delete")}
                </h3>
                <p className="text-sm text-muted-foreground">
                  {dl.status === "downloading" ? t("downloads.confirmCancel") : t("downloads.confirmDelete")}
                </p>
              </div>
              <div className="flex justify-end gap-2 px-6 pb-6">
                <Button variant="outline" size="sm" onClick={() => setShowConfirm(false)}>
                  {t("common.cancel")}
                </Button>
                <Button variant="destructive" size="sm" onClick={handleDelete}>
                  {dl.status === "downloading" ? t("downloads.cancel") : t("downloads.delete")}
                </Button>
              </div>
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  );
}

type AvailableSortKey = "seeders" | "missing" | "name" | "recent";

export function AvailableDownloadsSection({ latestFound, onDeleted }: { latestFound: LatestFoundPerLibraryDto[]; onDeleted: () => void }) {
  const { t } = useTranslation();
  const [sort, setSort] = useState<AvailableSortKey>("recent");
  const [filterLib, setFilterLib] = useState<string>("all");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);

  // Flatten all results with library info
  const allResults = latestFound.flatMap(lib =>
    lib.results.map(r => ({ ...r, library_id: lib.library_id, library_name: lib.library_name }))
  );

  const bestSeeders = (r: typeof allResults[0]) =>
    r.available_releases?.reduce((max, rel) => Math.max(max, rel.seeders ?? 0), 0) ?? 0;

  const newestDetectedAt = (r: typeof allResults[0]) =>
    r.available_releases?.reduce((newest, rel) => {
      const d = rel.detected_at ?? "";
      return d > newest ? d : newest;
    }, "") ?? r.updated_at ?? "";

  const filtered = allResults.filter(r => filterLib === "all" || r.library_id === filterLib);

  const sorted = [...filtered].sort((a, b) => {
    switch (sort) {
      case "seeders": return bestSeeders(b) - bestSeeders(a);
      case "missing": return b.missing_count - a.missing_count;
      case "name": return a.series_name.localeCompare(b.series_name);
      case "recent": return newestDetectedAt(b).localeCompare(newestDetectedAt(a));
      default: return 0;
    }
  });

  const libraries = latestFound.map(l => ({ id: l.library_id, name: l.library_name }));

  async function handleDeleteRelease(seriesId: string, releaseIdx: number, blacklist = false) {
    const key = `${seriesId}-${releaseIdx}`;
    setDeletingKey(key);
    try {
      const qs = blacklist ? `?release=${releaseIdx}&blacklist=true` : `?release=${releaseIdx}`;
      const resp = await fetch(`/api/available-downloads/${seriesId}${qs}`, { method: "DELETE" });
      if (resp.ok) {
        onDeleted();
        if (blacklist && showBlacklist) fetchBlacklist();
      }
    } finally {
      setDeletingKey(null);
    }
  }

  async function handleDeleteSeries(seriesId: string) {
    setDeletingKey(seriesId);
    try {
      const resp = await fetch(`/api/available-downloads/${seriesId}`, { method: "DELETE" });
      if (resp.ok) onDeleted();
    } finally {
      setDeletingKey(null);
    }
  }

  const sortOptions: { id: AvailableSortKey; label: string }[] = [
    { id: "recent", label: t("downloads.sortRecent") },
    { id: "seeders", label: t("downloads.sortSeeders") },
    { id: "missing", label: t("downloads.sortMissing") },
    { id: "name", label: t("downloads.sortName") },
  ];

  type BlacklistItem = { id: string; title: string; indexer: string | null; series_name: string | null };
  const [showBlacklist, setShowBlacklist] = useState(false);
  const [blacklistItems, setBlacklistItems] = useState<BlacklistItem[]>([]);
  const [loadingBlacklist, setLoadingBlacklist] = useState(false);

  async function fetchBlacklist() {
    setLoadingBlacklist(true);
    try {
      const resp = await fetch("/api/release-blacklist");
      if (resp.ok) setBlacklistItems(await resp.json());
    } catch { /* ignore */ }
    finally { setLoadingBlacklist(false); }
  }

  async function handleUnblacklist(itemId: string) {
    await fetch(`/api/release-blacklist/${itemId}`, { method: "DELETE" });
    setBlacklistItems((prev) => prev.filter((h) => h.id !== itemId));
  }

  return (
    <div className="mt-10">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 mb-4">
        <h2 className="text-xl font-bold text-foreground flex items-center gap-2">
          <Icon name="search" size="lg" />
          {t("downloads.availableTitle")}
          <span className="text-sm font-normal text-muted-foreground">({sorted.length})</span>
          <button
            onClick={() => { setShowBlacklist(!showBlacklist); if (!showBlacklist) fetchBlacklist(); }}
            className={`ml-2 px-2 py-1 text-xs rounded transition-colors ${showBlacklist ? "text-primary bg-primary/10" : "text-muted-foreground hover:text-foreground"}`}
            title={t("downloads.blacklisted")}
          >
            <Icon name="eye" size="sm" />
          </button>
        </h2>
        <div className="flex items-center gap-2">
          {libraries.length > 1 && (
            <select
              value={filterLib}
              onChange={(e) => setFilterLib(e.target.value)}
              className="text-xs border border-border rounded-lg px-2 py-1.5 bg-background"
            >
              <option value="all">{t("common.all")}</option>
              {libraries.map(l => <option key={l.id} value={l.id}>{l.name}</option>)}
            </select>
          )}
          <div className="flex gap-0.5">
            {sortOptions.map(s => (
              <button
                key={s.id}
                onClick={() => setSort(s.id)}
                className={`px-2.5 py-1.5 rounded-md text-xs font-medium border transition-colors ${
                  sort === s.id
                    ? "bg-primary/15 text-primary border-primary/30"
                    : "bg-card text-muted-foreground border-border hover:border-primary/30"
                }`}
              >
                {s.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {showBlacklist && (
        <div className="border border-border rounded-lg p-4 bg-card mb-4">
          <h3 className="text-sm font-semibold text-foreground mb-3">{t("downloads.blacklisted")}</h3>
          {loadingBlacklist ? (
            <div className="flex justify-center py-4"><Icon name="spinner" size="sm" className="animate-spin" /></div>
          ) : blacklistItems.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("downloads.noBlacklisted")}</p>
          ) : (
            <div className="space-y-1">
              {blacklistItems.map((item) => (
                <div key={item.id} className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-muted/50 text-xs">
                  <div className="min-w-0 flex-1">
                    <p className="font-medium truncate">{item.title}</p>
                    <p className="text-muted-foreground">{item.series_name} {item.indexer && `· ${item.indexer}`}</p>
                  </div>
                  <button
                    onClick={() => handleUnblacklist(item.id)}
                    className="text-xs px-2 py-1 rounded border border-border hover:bg-background transition-colors shrink-0"
                  >
                    {t("downloads.unblacklist")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <div className="border border-border rounded-xl overflow-hidden">
        {sorted.map((r) => {
          const isExpanded = expandedId === r.id;
          const topSeeders = bestSeeders(r);
          const releaseCount = r.available_releases?.length ?? 0;
          const failedReleaseCount = r.available_releases?.filter(rel => rel.has_failed).length ?? 0;

          return (
            <div key={r.id} className={`${isExpanded ? "bg-muted/20" : ""}`}>
              {/* Summary row */}
              <button
                type="button"
                onClick={() => setExpandedId(isExpanded ? null : r.id)}
                className={`w-full flex items-center gap-2 sm:gap-3 px-2.5 sm:px-3 py-2 text-left hover:bg-muted/30 transition-colors border-b border-border/40 ${isExpanded ? "bg-muted/20" : ""}`}
              >
                <Icon
                  name={isExpanded ? "chevronDown" : "chevronRight"}
                  size="sm"
                  className="text-muted-foreground shrink-0 !w-3.5 !h-3.5"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <Link
                      href={`/series/${r.series_id}`}
                      onClick={(e) => e.stopPropagation()}
                      className="text-sm font-semibold text-primary hover:underline truncate"
                    >
                      {r.series_name}
                    </Link>
                    {libraries.length > 1 && (
                      <span className="text-[10px] text-muted-foreground hidden sm:inline shrink-0">{r.library_name}</span>
                    )}
                  </div>
                  <div className="flex items-center gap-2 mt-0.5 text-[10px] text-muted-foreground">
                    <span>{releaseCount} release{releaseCount > 1 ? "s" : ""}</span>
                    {(() => {
                      const newest = newestDetectedAt(r);
                      return newest ? (
                        <span title={new Date(newest).toLocaleString()}>
                          {formatRelativeDate(newest)}
                        </span>
                      ) : null;
                    })()}
                    {r.available_releases && r.available_releases.length > 0 && (
                      <span className="hidden sm:inline truncate max-w-xs" title={r.available_releases[0].title}>
                        {r.available_releases[0].title}
                      </span>
                    )}
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  {failedReleaseCount > 0 && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-destructive/20 text-destructive" title={`${failedReleaseCount} failed`}>
                      {failedReleaseCount}!
                    </span>
                  )}
                  <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-warning/20 text-warning" title={`${r.missing_count} missing`}>
                    {r.missing_count} {t("downloads.missing")}
                  </span>
                  {topSeeders > 0 && (
                    <span className={`text-xs font-bold tabular-nums ${
                      topSeeders >= 10 ? "text-green-600" : topSeeders >= 3 ? "text-amber-600" : "text-red-500"
                    }`}>
                      {topSeeders}S
                    </span>
                  )}
                </div>
              </button>

              {/* Expanded: one flat line per release */}
              {isExpanded && r.available_releases && r.available_releases.length > 0 && (
                <div className="border-b border-border/40">
                  {r.available_releases.map((release, idx) => (
                    <div key={idx} className={`px-2 sm:px-3 py-0.5 pl-7 sm:pl-9 text-[10px] hover:bg-muted/20 border-b border-border/10 last:border-b-0 ${release.has_failed ? "bg-destructive/5" : ""}`}>
                      <div className="flex items-center gap-1.5 sm:gap-2">
                        {release.has_failed && (
                          <span className="px-1 py-px rounded bg-destructive/20 text-destructive font-medium shrink-0" title={t("downloads.failedBefore", { count: 1 })}>!</span>
                        )}
                        <div className="flex items-center gap-1 shrink-0">
                          {compressVolumes(release.matched_missing_volumes).map(range => (
                            <span key={range} className="px-1 py-px rounded bg-success/20 text-success font-medium">{range}</span>
                          ))}
                        </div>
                        {release.indexer && <span className="text-muted-foreground shrink-0">{release.indexer}</span>}
                        {release.seeders != null && (
                          <span className={`font-medium shrink-0 ${
                            release.seeders >= 10 ? "text-green-600" : release.seeders >= 3 ? "text-amber-600" : "text-red-500"
                          }`}>{release.seeders}S</span>
                        )}
                        <span className="text-muted-foreground shrink-0">{(release.size / 1024 / 1024).toFixed(0)}MB</span>
                        <div className="flex items-center gap-0.5 ml-auto shrink-0">
                          {release.download_url && (
                            <QbittorrentDownloadButton
                              downloadUrl={release.download_url}
                              releaseId={`${r.id}-${idx}`}
                              libraryId={r.library_id}
                              seriesName={r.series_name}
                              expectedVolumes={release.matched_missing_volumes}
                              allVolumes={release.all_volumes}
                            />
                          )}
                          <button
                            type="button"
                            onClick={() => handleDeleteRelease(r.id, idx, true)}
                            disabled={deletingKey === `${r.id}-${idx}`}
                            title={t("downloads.blacklist")}
                            className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-30 shrink-0"
                          >
                            <Icon name="x" size="sm" />
                          </button>
                          <button
                            type="button"
                            onClick={() => handleDeleteRelease(r.id, idx)}
                            disabled={deletingKey === `${r.id}-${idx}`}
                            title={t("downloads.delete")}
                            className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
                          >
                            {deletingKey === `${r.id}-${idx}`
                              ? <Icon name="spinner" size="sm" className="animate-spin" />
                              : <Icon name="trash" size="sm" />}
                          </button>
                        </div>
                      </div>
                      <p className="text-[9px] text-muted-foreground/60 truncate pl-0.5" title={release.title}>{release.title}</p>
                    </div>
                  ))}
                  <div className="flex justify-end px-2 sm:px-3 py-0.5 pl-7 sm:pl-9">
                    <button
                      type="button"
                      onClick={() => handleDeleteSeries(r.id)}
                      disabled={deletingKey === r.id}
                      className="text-[10px] text-muted-foreground hover:text-destructive transition-colors disabled:opacity-30"
                    >
                      {deletingKey === r.id
                        ? <Icon name="spinner" size="sm" className="animate-spin !w-3 !h-3" />
                        : t("downloads.dismissAll")}
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

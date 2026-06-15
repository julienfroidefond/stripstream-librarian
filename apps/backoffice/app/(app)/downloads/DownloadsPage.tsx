"use client";

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import {
  AvailableReleaseDto,
  TorrentDownloadDto,
  LatestFoundPerLibraryDto,
  TelegramAvailableBookDto,
  TelegramAvailableGroupDto,
  TelegramDownloadItemDto,
} from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle, Button, Icon, toast } from "@/app/components/ui";
import { QbittorrentProvider, QbittorrentDownloadButton } from "@/app/components/QbittorrentDownloadButton";
import { useTranslation } from "@/lib/i18n/context";
import { compressVolumes } from "@/lib/volumeRanges";
import type { TranslationKey } from "@/lib/i18n/fr";

type TFunction = (key: TranslationKey, vars?: Record<string, string | number>) => string;


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
    queued:            "downloads.status.queued",
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
    case "queued":      return "bg-muted/50 text-muted-foreground";
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
  initialTelegramAvailable?: TelegramAvailableGroupDto[];
  initialTelegramDownloads?: TelegramDownloadItemDto[];
}

const PAGE_SIZE = 10;
type StatusFilter = "all" | "active" | "imported" | "error";

type DownloadItem =
  | { kind: "torrent"; data: TorrentDownloadDto }
  | { kind: "telegram"; data: TelegramDownloadItemDto };

type UnifiedAvailableSource =
  | {
      kind: "prowlarr";
      entryId: string;
      releaseIndex: number;
      release: AvailableReleaseDto;
      detectedAt: string;
    }
  | {
      kind: "telegram";
      book: TelegramAvailableBookDto;
    };

type UnifiedAvailableGroup = {
  key: string;
  series_id: string | null;
  series_name: string;
  library_id: string;
  library_name: string;
  missing_count: number;
  owned_volumes: number[];
  updated_at: string;
  sources: UnifiedAvailableSource[];
};

const ACTIVE_STATUSES = new Set(["queued", "downloading", "completed", "importing"]);

function mergeDownloads(torrents: TorrentDownloadDto[], tg: TelegramDownloadItemDto[]): DownloadItem[] {
  const items: DownloadItem[] = [
    ...torrents.map(d => ({ kind: "torrent" as const, data: d })),
    ...tg.map(d => ({ kind: "telegram" as const, data: d })),
  ];
  return items.sort((a, b) => {
    const aActive = ACTIVE_STATUSES.has(a.data.status);
    const bActive = ACTIVE_STATUSES.has(b.data.status);
    if (aActive !== bActive) return aActive ? -1 : 1;
    if (aActive) {
      // Active: oldest created_at first — stable, no swapping during progress updates
      return new Date(a.data.created_at).getTime() - new Date(b.data.created_at).getTime();
    } else {
      // Inactive: most recently updated first — stable since updated_at doesn't change after import
      return new Date(b.data.updated_at).getTime() - new Date(a.data.updated_at).getTime();
    }
  });
}

function itemMatchesFilter(item: DownloadItem, filter: string): boolean {
  if (filter === "all") return true;
  if (item.kind === "torrent") {
    if (filter === "active") return STATUS_ACTIVE.has(item.data.status);
    return item.data.status === filter;
  } else {
    if (filter === "active") return item.data.status === "queued" || item.data.status === "downloading";
    if (filter === "imported") return item.data.status === "imported";
    if (filter === "error") return item.data.status === "failed";
    return false;
  }
}

function newestDate(a: string, b: string): string {
  return a.localeCompare(b) >= 0 ? a : b;
}

function unifiedGroupKey(libraryId: string, seriesId: string | null, seriesName: string): string {
  return `${libraryId}|${seriesId ?? seriesName.toLocaleLowerCase()}`;
}

function getSourceVolumes(source: UnifiedAvailableSource): number[] {
  if (source.kind === "prowlarr") return source.release.matched_missing_volumes;
  return source.book.volume_number == null ? [] : [source.book.volume_number];
}

function buildUnifiedAvailableGroups(
  latestFound: LatestFoundPerLibraryDto[],
  telegramAvailable: TelegramAvailableGroupDto[],
): UnifiedAvailableGroup[] {
  const groups = new Map<string, UnifiedAvailableGroup>();

  const ensureGroup = (
    libraryId: string,
    libraryName: string,
    seriesId: string | null,
    seriesName: string,
    missingCount: number,
    ownedVolumes: number[],
    updatedAt: string,
  ) => {
    const key = unifiedGroupKey(libraryId, seriesId, seriesName);
    const existing = groups.get(key);
    if (existing) {
      existing.missing_count = Math.max(existing.missing_count, missingCount);
      existing.updated_at = newestDate(existing.updated_at, updatedAt);
      existing.owned_volumes = Array.from(new Set([...existing.owned_volumes, ...ownedVolumes]));
      return existing;
    }

    const group: UnifiedAvailableGroup = {
      key,
      series_id: seriesId,
      series_name: seriesName,
      library_id: libraryId,
      library_name: libraryName,
      missing_count: missingCount,
      owned_volumes: ownedVolumes,
      updated_at: updatedAt,
      sources: [],
    };
    groups.set(key, group);
    return group;
  };

  latestFound.forEach(lib => {
    lib.results.forEach(result => {
      const releases = result.available_releases ?? [];
      if (releases.length === 0) return;
      const group = ensureGroup(
        lib.library_id,
        lib.library_name,
        result.series_id,
        result.series_name,
        result.missing_count,
        [],
        result.updated_at,
      );
      releases.forEach((release, releaseIndex) => {
        const detectedAt = release.detected_at ?? result.updated_at;
        group.sources.push({ kind: "prowlarr", entryId: result.id, releaseIndex, release, detectedAt });
        group.updated_at = newestDate(group.updated_at, detectedAt);
      });
    });
  });

  telegramAvailable.forEach(tgGroup => {
    const owned = new Set(tgGroup.owned_volumes);
    const books = tgGroup.books.filter(book => !(book.volume_number != null && owned.has(book.volume_number)));
    if (books.length === 0) return;
    if (tgGroup.series_missing_count === 0 && tgGroup.owned_volumes.length > 0) return;

    const newestBookDate = books.reduce((latest, book) => newestDate(latest, book.created_at), books[0]?.created_at ?? "");
    const group = ensureGroup(
      tgGroup.library_id,
      tgGroup.library_name,
      tgGroup.series_id,
      tgGroup.series_name,
      tgGroup.series_missing_count,
      tgGroup.owned_volumes,
      newestBookDate,
    );

    const existingTelegramIds = new Set(
      group.sources
        .filter((source): source is Extract<UnifiedAvailableSource, { kind: "telegram" }> => source.kind === "telegram")
        .map(source => source.book.id),
    );
    books.forEach(book => {
      if (!existingTelegramIds.has(book.id)) {
        group.sources.push({ kind: "telegram", book });
      }
    });
  });

  return Array.from(groups.values()).filter(group => group.sources.length > 0);
}

export function DownloadsPage({ initialDownloads, initialLatestFound, qbConfigured, initialTelegramAvailable = [], initialTelegramDownloads = [] }: DownloadsPageProps) {
  const { t } = useTranslation();
  const [downloads, setDownloads] = useState<TorrentDownloadDto[]>(initialDownloads);
  const [telegramDownloads, setTelegramDownloads] = useState<TelegramDownloadItemDto[]>(initialTelegramDownloads);
  const [latestFound, setLatestFound] = useState<LatestFoundPerLibraryDto[]>(initialLatestFound);
  const [telegramAvailable, setTelegramAvailable] = useState<TelegramAvailableGroupDto[]>(initialTelegramAvailable);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [page, setPage] = useState(1);

  const refresh = useCallback(async (showSpinner = true) => {
    if (showSpinner) setIsRefreshing(true);
    try {
      const [dlResp, lfResp, tgResp, tgDlResp] = await Promise.all([
        fetch("/api/torrent-downloads"),
        fetch("/api/download-detection/latest-found"),
        fetch("/api/telegram-monitor/available"),
        fetch("/api/telegram-monitor/downloads"),
      ]);
      if (dlResp.ok) setDownloads(await dlResp.json());
      if (lfResp.ok) setLatestFound(await lfResp.json());
      if (tgResp.ok) setTelegramAvailable(await tgResp.json());
      if (tgDlResp.ok) setTelegramDownloads(await tgDlResp.json());
    } finally {
      if (showSpinner) setIsRefreshing(false);
    }
  }, []);

  const merged = mergeDownloads(downloads, telegramDownloads);

  // Auto-refresh every 2s while there are active downloads or telegram downloads in progress
  const hasActive = merged.some(item =>
    item.kind === "torrent" ? STATUS_ACTIVE.has(item.data.status) : item.data.status === "downloading"
  );
  useEffect(() => {
    if (!hasActive) return;
    const id = setInterval(() => refresh(false), 2000);
    return () => clearInterval(id);
  }, [hasActive, refresh]);

  const statusFilters: Array<{ id: StatusFilter; label: string }> = [
    { id: "all", label: t("common.all") },
    { id: "active", label: t("downloads.filterActive") },
    { id: "imported", label: t("downloads.status.imported") },
    { id: "error", label: t("downloads.status.error") },
  ];

  const visible = merged.filter(item => itemMatchesFilter(item, statusFilter));
  const totalPages = Math.ceil(visible.length / PAGE_SIZE);
  const paged = visible.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  // Reset to page 1 when filters change
  const handleStatusFilterChange = (id: StatusFilter) => { setStatusFilter(id); setPage(1); };

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
        {statusFilters.map(f => (
          <button
            key={f.id}
            onClick={() => handleStatusFilterChange(f.id)}
            className={`px-3 sm:px-4 py-2 text-xs sm:text-sm font-medium border-b-2 transition-colors -mb-px whitespace-nowrap ${
              statusFilter === f.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
            }`}
          >
            {f.label}
            {f.id !== "all" && (
              <span className="ml-1 sm:ml-1.5 text-[10px] sm:text-xs opacity-60">
                {merged.filter(item => itemMatchesFilter(item, f.id)).length}
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
            {paged.map(item => item.kind === "torrent"
              ? <DownloadRow key={`t-${item.data.id}`} dl={item.data} onDeleted={() => refresh(false)} onRetried={() => refresh(false)} />
              : <TelegramDownloadRow key={`tg-${item.data.id}`} item={item.data} onRefresh={() => refresh(false)} />
            )}
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

      {(latestFound.length > 0 || telegramAvailable.length > 0) && (
        <QbittorrentProvider initialConfigured={qbConfigured} onDownloadStarted={() => refresh(false)}>
          <AvailableDownloadsSection
            latestFound={latestFound}
            telegramAvailable={telegramAvailable}
            onRefresh={() => refresh(false)}
            onTelegramDownloaded={(bookId) => {
              setTelegramAvailable(prev => prev.map(group => ({
                ...group,
                books: group.books.map(book => book.id === bookId ? { ...book, status: "queued" } : book),
              })));
            }}
          />
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

function TelegramDownloadRow({ item, onRefresh }: { item: TelegramDownloadItemDto; onRefresh: () => void }) {
  const { t } = useTranslation();
  const [retrying, setRetrying] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);

  async function handleRetry() {
    setRetrying(true);
    try {
      const resp = await fetch(`/api/telegram-monitor/books/${item.id}/download`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({}) });
      if (resp.ok) {
        onRefresh();
      } else {
        const body = await resp.json().catch(() => ({}));
        toast(body?.error ?? `Error ${resp.status}`, "error");
      }
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setRetrying(false);
    }
  }

  async function handleDelete() {
    setDeleting(true);
    setShowConfirm(false);
    try {
      const resp = await fetch(`/api/telegram-monitor/books/${item.id}`, { method: "DELETE" });
      if (resp.ok) onRefresh();
    } finally {
      setDeleting(false);
    }
  }

  // Map telegram statuses to qBit status classes/labels
  const tgStatus = item.status === "failed" ? "error" : item.status;
  const volLabel = item.volume_number != null ? `T${String(item.volume_number).padStart(2, "0")}` : null;

  const statusIcon = item.status === "imported"
    ? <Icon name="check" size="sm" className="text-success" />
    : item.status === "downloading"
    ? <Icon name="download" size="sm" className="text-primary" />
    : item.status === "queued"
    ? <Icon name="clock" size="sm" className="text-muted-foreground" />
    : <Icon name="warning" size="sm" className="text-destructive" />;

  return (
    <>
      <div className="flex items-start sm:items-center gap-2 sm:gap-3 px-3 py-2 rounded-lg border border-border/40 bg-card hover:bg-accent/30 transition-colors">
        <div className="mt-0.5 sm:mt-0">{statusIcon}</div>

        <div className="flex-1 min-w-0">
          {/* Desktop */}
          <div className="hidden sm:flex items-center gap-2">
            {item.series_id
              ? <Link href={`/series/${item.series_id}`} className="text-sm font-medium text-primary hover:underline truncate">{item.series_name ?? item.filename}</Link>
              : <span className="text-sm font-medium text-foreground truncate">{item.series_name ?? item.filename}</span>
            }
            <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full ${statusClass(tgStatus)}`}>
              {statusLabel(tgStatus, t)}
            </span>
            {volLabel && <span className="text-[11px] text-muted-foreground">{volLabel}</span>}
            <span className="text-[11px] text-muted-foreground truncate">@{item.channel_username}</span>
          </div>
          {/* Mobile */}
          <div className="sm:hidden">
            <div className="flex items-center gap-1.5">
              {item.series_id
                ? <Link href={`/series/${item.series_id}`} className="text-sm font-medium text-primary hover:underline truncate">{item.series_name ?? item.filename}</Link>
                : <span className="text-sm font-medium text-foreground truncate">{item.series_name ?? item.filename}</span>
              }
              <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full shrink-0 ${statusClass(tgStatus)}`}>
                {statusLabel(tgStatus, t)}
              </span>
            </div>
            <div className="flex items-center gap-2 mt-0.5 text-[11px] text-muted-foreground">
              {volLabel && <span>{volLabel}</span>}
              <span className="tabular-nums">{formatDate(item.updated_at)}</span>
            </div>
          </div>
          {/* Filename secondary */}
          <p className="hidden sm:block text-[11px] text-muted-foreground truncate mt-0.5">{item.filename}</p>
          {/* Progress bar for active downloads */}
          {item.status === "downloading" && item.file_size && item.file_size > 0 && (
            <div className="mt-1.5 space-y-0.5">
              <div className="w-full bg-muted/50 rounded-full h-1.5 overflow-hidden">
                <div
                  className="bg-primary h-1.5 rounded-full transition-all duration-500"
                  style={{ width: `${Math.min(100, Math.round((item.bytes_downloaded / item.file_size) * 100))}%` }}
                />
              </div>
              <p className="text-[10px] text-muted-foreground tabular-nums">
                {formatSize(item.bytes_downloaded)} / {formatSize(item.file_size)}
                {" · "}{Math.min(100, Math.round((item.bytes_downloaded / item.file_size) * 100))}%
              </p>
            </div>
          )}
          {item.error_message && (
            <p className="text-[11px] text-destructive truncate mt-0.5" title={item.error_message}>{item.error_message}</p>
          )}
        </div>

        <span className="text-[10px] text-muted-foreground shrink-0 tabular-nums hidden sm:block">{formatDate(item.updated_at)}</span>

        {(item.status === "failed" || item.status === "imported") && (
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
          disabled={deleting}
          className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
          title={(item.status === "queued" || item.status === "downloading") ? t("downloads.cancel") : t("downloads.delete")}
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
                  {(item.status === "queued" || item.status === "downloading") ? t("downloads.cancel") : t("downloads.delete")}
                </h3>
                <p className="text-sm text-muted-foreground">
                  {item.status === "downloading" ? t("downloads.confirmCancel") : t("downloads.confirmDelete")}
                </p>
              </div>
              <div className="flex justify-end gap-2 px-6 pb-6">
                <Button variant="outline" size="sm" onClick={() => setShowConfirm(false)}>
                  {t("common.cancel")}
                </Button>
                <Button variant="destructive" size="sm" onClick={handleDelete}>
                  {(item.status === "queued" || item.status === "downloading") ? t("downloads.cancel") : t("downloads.delete")}
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
type AvailableSourceFilter = "all" | "prowlarr" | "telegram";

function groupMatchesSourceFilter(group: UnifiedAvailableGroup, sourceFilter: AvailableSourceFilter): boolean {
  if (sourceFilter === "all") return true;
  return group.sources.some(source => source.kind === sourceFilter);
}

export function AvailableDownloadsSection({
  latestFound,
  telegramAvailable,
  onRefresh,
  onTelegramDownloaded,
}: {
  latestFound: LatestFoundPerLibraryDto[];
  telegramAvailable: TelegramAvailableGroupDto[];
  onRefresh: () => void;
  onTelegramDownloaded?: (bookId: string) => void;
}) {
  const { t } = useTranslation();
  const [sort, setSort] = useState<AvailableSortKey>("recent");
  const [filterLib, setFilterLib] = useState<string>("all");
  const [sourceFilter, setSourceFilter] = useState<AvailableSourceFilter>("all");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);
  const [downloadingTelegramIds, setDownloadingTelegramIds] = useState<Set<string>>(new Set());
  const [dismissingTelegramIds, setDismissingTelegramIds] = useState<Set<string>>(new Set());

  const allResults = buildUnifiedAvailableGroups(latestFound, telegramAvailable);

  const bestSeeders = (r: UnifiedAvailableGroup) =>
    r.sources.reduce((max, source) => {
      if (source.kind !== "prowlarr") return max;
      return Math.max(max, source.release.seeders ?? 0);
    }, 0);

  const newestDetectedAt = (r: UnifiedAvailableGroup) => r.updated_at;

  const filtered = allResults.filter(r =>
    (filterLib === "all" || r.library_id === filterLib) &&
    groupMatchesSourceFilter(r, sourceFilter)
  );

  const sorted = [...filtered].sort((a, b) => {
    switch (sort) {
      case "seeders": return bestSeeders(b) - bestSeeders(a);
      case "missing": return b.missing_count - a.missing_count;
      case "name": return a.series_name.localeCompare(b.series_name);
      case "recent": return newestDetectedAt(b).localeCompare(newestDetectedAt(a));
      default: return 0;
    }
  });

  const libraries = Array.from(
    new Map(allResults.map(r => [r.library_id, r.library_name])).entries()
  ).map(([id, name]) => ({ id, name }));

  async function handleDeleteRelease(seriesId: string, releaseIdx: number, blacklist = false) {
    const key = `${seriesId}-${releaseIdx}`;
    setDeletingKey(key);
    try {
      const qs = blacklist ? `?release=${releaseIdx}&blacklist=true` : `?release=${releaseIdx}`;
      const resp = await fetch(`/api/available-downloads/${seriesId}${qs}`, { method: "DELETE" });
      if (resp.ok) {
        onRefresh();
        if (blacklist && showBlacklist) fetchBlacklist();
      }
    } finally {
      setDeletingKey(null);
    }
  }

  async function handleDeleteProwlarrEntries(group: UnifiedAvailableGroup) {
    const ids = Array.from(new Set(
      group.sources
        .filter((source): source is Extract<UnifiedAvailableSource, { kind: "prowlarr" }> => source.kind === "prowlarr")
        .map(source => source.entryId),
    ));
    setDeletingKey(group.key);
    try {
      await Promise.all(ids.map(id => fetch(`/api/available-downloads/${id}`, { method: "DELETE" })));
      onRefresh();
    } finally {
      setDeletingKey(null);
    }
  }

  async function handleTelegramDownload(bookId: string) {
    setDownloadingTelegramIds(prev => new Set(prev).add(bookId));
    try {
      const resp = await fetch(`/api/telegram-monitor/books/${bookId}/download`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      if (resp.ok) onTelegramDownloaded?.(bookId);
    } finally {
      setDownloadingTelegramIds(prev => { const next = new Set(prev); next.delete(bookId); return next; });
    }
  }

  async function handleTelegramDismiss(bookId: string) {
    setDismissingTelegramIds(prev => new Set(prev).add(bookId));
    try {
      const resp = await fetch(`/api/telegram-monitor/books/${bookId}`, { method: "DELETE" });
      if (resp.ok) onRefresh();
    } finally {
      setDismissingTelegramIds(prev => { const next = new Set(prev); next.delete(bookId); return next; });
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
        <div className="flex items-center gap-2 flex-wrap justify-end">
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
          <select
            value={sourceFilter}
            onChange={(e) => setSourceFilter(e.target.value as AvailableSourceFilter)}
            aria-label={t("downloads.filterSource")}
            className="text-xs border border-border rounded-lg px-2 py-1.5 bg-background"
          >
            <option value="all">{t("common.all")}</option>
            <option value="prowlarr">{t("downloads.sourceProwlarr")}</option>
            <option value="telegram">{t("downloads.sourceTelegram")}</option>
          </select>
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
          const isExpanded = expandedId === r.key;
          const visibleSources = sourceFilter === "all"
            ? r.sources
            : r.sources.filter(source => source.kind === sourceFilter);
          const topSeeders = bestSeeders({ ...r, sources: visibleSources });
          const prowlarrCount = visibleSources.filter(source => source.kind === "prowlarr").length;
          const telegramCount = visibleSources.filter(source => source.kind === "telegram").length;
          const failedReleaseCount = visibleSources.filter(source =>
            source.kind === "prowlarr" ? source.release.has_failed : source.book.status === "failed"
          ).length;

          return (
            <div key={r.key} className={`${isExpanded ? "bg-muted/20" : ""}`}>
              {/* Summary row */}
              <button
                type="button"
                onClick={() => setExpandedId(isExpanded ? null : r.key)}
                className={`w-full flex flex-wrap sm:flex-nowrap items-center gap-x-2 gap-y-1 sm:gap-3 px-2.5 sm:px-3 py-2 text-left hover:bg-muted/30 transition-colors border-b border-border/40 ${isExpanded ? "bg-muted/20" : ""}`}
              >
                <Icon
                  name={isExpanded ? "chevronDown" : "chevronRight"}
                  size="sm"
                  className="text-muted-foreground shrink-0 !w-3.5 !h-3.5"
                />
                <div className="flex-1 min-w-0 flex items-center gap-2 overflow-hidden">
                  <Link
                    href={`/series/${r.series_id}`}
                    onClick={(e) => e.stopPropagation()}
                    className="text-sm font-semibold text-primary hover:underline truncate shrink"
                  >
                    {r.series_name}
                  </Link>
                  {libraries.length > 1 && (
                    <span className="text-[10px] text-muted-foreground hidden sm:inline shrink-0">{r.library_name}</span>
                  )}
                </div>

                <div className="flex w-full sm:w-auto items-center justify-end gap-2 shrink-0 pl-5 sm:pl-0">
                  {prowlarrCount > 0 && (
                    <span className="text-[10px] text-muted-foreground shrink-0">
                      {prowlarrCount} Prowlarr
                    </span>
                  )}
                  {telegramCount > 0 && (
                    <span className="text-[10px] text-sky-600 shrink-0">
                      {telegramCount} TG
                    </span>
                  )}
                  {(() => {
                    const newest = newestDetectedAt(r);
                    return newest ? (
                      <span className="text-[10px] text-muted-foreground shrink-0 tabular-nums">{formatDate(newest)}</span>
                    ) : null;
                  })()}
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

              {/* Expanded: one compact line per release */}
              {isExpanded && visibleSources.length > 0 && (
                <div className="border-b border-border/40">
                  {[...visibleSources].sort((a, b) => {
                    const aVol = Math.min(...getSourceVolumes(a), 9999);
                    const bVol = Math.min(...getSourceVolumes(b), 9999);
                    if (aVol !== bVol) return aVol - bVol;
                    return a.kind.localeCompare(b.kind);
                  }).map((source) => source.kind === "prowlarr" ? (
                    <div
                      key={`prowlarr-${source.entryId}-${source.releaseIndex}`}
                      className={`flex items-center gap-1.5 sm:gap-2 px-2 sm:px-3 py-1.5 pl-7 sm:pl-9 text-[11px] hover:bg-muted/20 border-b border-border/10 last:border-b-0 transition-colors ${source.release.has_failed ? "bg-destructive/5 hover:bg-destructive/10" : ""}`}
                    >
                      {source.release.has_failed && (
                        <span className="px-1.5 py-px rounded bg-destructive/20 text-destructive font-medium shrink-0" title={t("downloads.failedBefore", { count: 1 })}>!</span>
                      )}
                      <span className="px-1.5 py-px rounded bg-primary/10 text-primary font-medium shrink-0">Prowlarr</span>
                      <div className="flex items-center gap-1 shrink-0">
                        {compressVolumes(source.release.matched_missing_volumes).map(range => (
                          <span key={range} className="px-1.5 py-px rounded bg-success/20 text-success font-medium tabular-nums">{range}</span>
                        ))}
                      </div>
                      <span className={`flex-1 min-w-0 truncate font-mono ${source.release.has_failed ? "text-destructive/80" : "text-muted-foreground"}`} title={source.release.title}>
                        {source.release.title}
                      </span>
                      {source.release.indexer && <span className="text-muted-foreground shrink-0 hidden sm:inline">{source.release.indexer}</span>}
                      {source.release.seeders != null && (
                        <span className={`font-medium shrink-0 tabular-nums ${
                          source.release.seeders >= 10 ? "text-green-600" : source.release.seeders >= 3 ? "text-amber-600" : "text-red-500"
                        }`}>{source.release.seeders}S</span>
                      )}
                      <span className="text-muted-foreground shrink-0 tabular-nums">{(source.release.size / 1024 / 1024).toFixed(0)}MB</span>
                      <div className="flex items-center gap-0.5 ml-auto shrink-0">
                        {source.release.download_url && (
                          <QbittorrentDownloadButton
                            downloadUrl={source.release.download_url}
                            releaseId={`${source.entryId}-${source.releaseIndex}`}
                            libraryId={r.library_id}
                            seriesName={r.series_name}
                            expectedVolumes={source.release.matched_missing_volumes}
                            allVolumes={source.release.all_volumes}
                          />
                        )}
                        <button
                          type="button"
                          onClick={() => handleDeleteRelease(source.entryId, source.releaseIndex, true)}
                          disabled={deletingKey === `${source.entryId}-${source.releaseIndex}`}
                          title={t("downloads.blacklist")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-30 shrink-0"
                        >
                          <Icon name="x" size="sm" />
                        </button>
                        <button
                          type="button"
                          onClick={() => handleDeleteRelease(source.entryId, source.releaseIndex)}
                          disabled={deletingKey === `${source.entryId}-${source.releaseIndex}`}
                          title={t("downloads.delete")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
                        >
                          {deletingKey === `${source.entryId}-${source.releaseIndex}`
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : <Icon name="trash" size="sm" />}
                        </button>
                      </div>
                    </div>
                  ) : (
                    <div
                      key={`telegram-${source.book.id}`}
                      className={`flex items-center gap-1.5 sm:gap-2 px-2 sm:px-3 py-1.5 pl-7 sm:pl-9 text-[11px] border-b border-border/10 last:border-b-0 transition-colors ${source.book.status === "failed" ? "bg-destructive/5 hover:bg-destructive/10" : "hover:bg-muted/20"}`}
                    >
                      <span className="px-1.5 py-px rounded bg-sky-500/15 text-sky-600 font-medium shrink-0">TG</span>
                      {source.book.volume_number != null ? (
                        <span className={`px-1.5 py-px rounded font-medium shrink-0 tabular-nums ${source.book.status === "failed" ? "bg-destructive/20 text-destructive" : "bg-success/20 text-success"}`}>
                          T{String(source.book.volume_number).padStart(2, "0")}
                        </span>
                      ) : (
                        <span className="px-1.5 py-px rounded bg-muted/50 text-muted-foreground text-xs shrink-0">?</span>
                      )}
                      <span className={`flex-1 min-w-0 truncate ${source.book.status === "failed" ? "text-destructive/80" : "text-muted-foreground"}`} title={source.book.filename}>
                        {source.book.filename}
                      </span>
                      <span className="text-muted-foreground shrink-0 hidden sm:inline">@{source.book.channel_username}</span>
                      {source.book.file_size && <span className="text-muted-foreground shrink-0">{formatSize(source.book.file_size)}</span>}
                      <div className="flex items-center gap-0.5 ml-auto shrink-0">
                        {(source.book.status === "queued" || source.book.status === "downloading") ? (
                          <span className={`inline-flex items-center justify-center w-7 h-7 ${source.book.status === "queued" ? "text-muted-foreground" : "text-primary"}`}>
                            {source.book.status === "queued"
                              ? <Icon name="clock" size="sm" />
                              : <Icon name="spinner" size="sm" className="animate-spin" />}
                          </span>
                        ) : (
                        <button
                          type="button"
                          onClick={() => handleTelegramDownload(source.book.id)}
                          disabled={downloadingTelegramIds.has(source.book.id)}
                          title={source.book.status === "failed" ? t("downloads.retry") : t("telegramMonitor.download")}
                          className={`inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-30 ${source.book.status === "failed" ? "text-destructive hover:text-foreground hover:bg-muted" : "text-muted-foreground hover:text-success hover:bg-success/10"}`}
                        >
                          {downloadingTelegramIds.has(source.book.id)
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : source.book.status === "failed" ? <Icon name="refresh" size="sm" /> : <Icon name="download" size="sm" />}
                        </button>
                        )}
                        <button
                          type="button"
                          onClick={() => handleTelegramDismiss(source.book.id)}
                          disabled={dismissingTelegramIds.has(source.book.id)}
                          title={t("downloads.delete")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30"
                        >
                          {dismissingTelegramIds.has(source.book.id)
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : <Icon name="trash" size="sm" />}
                        </button>
                      </div>
                    </div>
                  ))}
                  {prowlarrCount > 0 && (
                  <div className="flex justify-end px-2 sm:px-3 py-0.5 pl-7 sm:pl-9">
                    <button
                      type="button"
                      onClick={() => handleDeleteProwlarrEntries(r)}
                      disabled={deletingKey === r.key}
                      className="text-[10px] text-muted-foreground hover:text-destructive transition-colors disabled:opacity-30"
                    >
                      {deletingKey === r.key
                        ? <Icon name="spinner" size="sm" className="animate-spin !w-3 !h-3" />
                        : t("downloads.dismissAll")}
                    </button>
                  </div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function formatSize(bytes: number | null) {
  if (!bytes) return "";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

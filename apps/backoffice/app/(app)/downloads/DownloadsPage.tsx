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
    downloading: "downloads.status.downloading",
    completed:   "downloads.status.completed",
    importing:   "downloads.status.importing",
    imported:    "downloads.status.imported",
    error:       "downloads.status.error",
  };
  return t(map[status] ?? "downloads.status.error");
}

function statusClass(status: string): string {
  switch (status) {
    case "downloading": return "bg-primary/10 text-primary";
    case "completed":   return "bg-warning/10 text-warning";
    case "importing":   return "bg-primary/10 text-primary";
    case "imported":    return "bg-success/10 text-success";
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
          <Icon name="download" size="lg" className="sm:hidden" />
          <Icon name="download" size="xl" className="hidden sm:block" />
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
              <DownloadRow key={dl.id} dl={dl} onDeleted={() => refresh(false)} />
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
          <div className="mt-10">
            <h2 className="text-xl font-bold text-foreground mb-4 flex items-center gap-2">
              <Icon name="search" size="lg" />
              {t("downloads.availableTitle")}
            </h2>
            <div className="space-y-6">
              {latestFound.map(lib => (
                <AvailableLibraryCard key={lib.library_id} lib={lib} onDeleted={() => refresh(false)} />
              ))}
            </div>
          </div>
        </QbittorrentProvider>
      )}
    </>
  );
}

function DownloadRow({ dl, onDeleted }: { dl: TorrentDownloadDto; onDeleted: () => void }) {
  const { t } = useTranslation();
  const [deleting, setDeleting] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const importedCount = Array.isArray(dl.imported_files) ? dl.imported_files.length : 0;

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

  const statusIcon = dl.status === "importing" ? (
    <Icon name="spinner" size="sm" className="animate-spin text-primary" />
  ) : dl.status === "imported" ? (
    <Icon name="check" size="sm" className="text-success" />
  ) : dl.status === "error" ? (
    <Icon name="warning" size="sm" className="text-destructive" />
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

function AvailableLibraryCard({ lib, onDeleted }: { lib: LatestFoundPerLibraryDto; onDeleted: () => void }) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(true);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);
  const displayResults = collapsed ? lib.results.slice(0, 5) : lib.results;

  async function handleDeleteRelease(seriesId: string, releaseIdx: number) {
    const key = `${seriesId}-${releaseIdx}`;
    setDeletingKey(key);
    try {
      const resp = await fetch(`/api/available-downloads/${seriesId}?release=${releaseIdx}`, { method: "DELETE" });
      if (resp.ok) onDeleted();
    } finally {
      setDeletingKey(null);
    }
  }

  return (
    <Card>
      <CardHeader className="pb-3 px-3 sm:px-6">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-1">
          <CardTitle className="text-sm sm:text-base">{lib.library_name}</CardTitle>
          <span className="text-[10px] sm:text-xs text-muted-foreground">
            {t("downloads.detectedSeries", { count: lib.results.length })}
          </span>
        </div>
      </CardHeader>
      <CardContent className="space-y-2 px-3 sm:px-6">
        {displayResults.map(r => (
          <div key={r.id} className="rounded-lg border border-border/40 bg-background/60 p-2 sm:p-3">
            <div className="flex items-center justify-between gap-2 mb-1.5">
              <Link
                href={`/series/${r.series_id ?? encodeURIComponent(r.series_name)}`}
                className="font-semibold text-xs sm:text-sm text-primary hover:underline truncate"
              >
                {r.series_name}
              </Link>
              <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap bg-warning/20 text-warning shrink-0">
                {r.missing_count} {t("downloads.missing")}
              </span>
            </div>
            {r.available_releases && r.available_releases.length > 0 && (
              <div className="space-y-1.5 sm:space-y-1">
                {groupReleasesByTitle(r.available_releases).map((group) => (
                  <div key={group.title} className="rounded bg-muted/30 overflow-hidden">
                    {/* Title + matched volumes (shown once) */}
                    <div className="py-1 px-2">
                      <p className="text-[11px] sm:text-xs font-mono text-foreground break-all">{group.title}</p>
                      <div className="flex flex-wrap items-center gap-1 mt-1">
                        {compressVolumes(group.items[0].matched_missing_volumes).map(range => (
                          <span key={range} className="text-[10px] px-1 py-0.5 rounded-full bg-success/20 text-success font-medium">{range}</span>
                        ))}
                      </div>
                    </div>
                    {/* Sources */}
                    {group.items.map((release, si) => {
                      const idx = group.originalIndices[si];
                      return (
                      <div key={idx} className={`flex items-center gap-2 sm:gap-3 py-0.5 px-2 ${si > 0 ? "border-t border-border/20" : ""}`}>
                        <div className="flex items-center gap-2 sm:gap-3 flex-1 min-w-0 flex-wrap">
                          {release.indexer && <span className="text-[10px] text-muted-foreground">{release.indexer}</span>}
                          {release.seeders != null && (
                            <span className="text-[10px] text-success font-medium">{release.seeders}S</span>
                          )}
                          <span className="text-[10px] text-muted-foreground">{(release.size / 1024 / 1024).toFixed(0)} MB</span>
                        </div>
                        <div className="flex items-center gap-1 shrink-0">
                          {release.download_url && (
                            <QbittorrentDownloadButton
                              downloadUrl={release.download_url}
                              releaseId={`${r.id}-${idx}`}
                              libraryId={lib.library_id}
                              seriesName={r.series_name}
                              expectedVolumes={release.matched_missing_volumes}
                              allVolumes={release.all_volumes}
                            />
                          )}
                          <button
                            type="button"
                            onClick={() => handleDeleteRelease(r.id, idx)}
                            disabled={deletingKey === `${r.id}-${idx}`}
                            className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30"
                            title={t("downloads.delete")}
                          >
                            {deletingKey === `${r.id}-${idx}`
                              ? <Icon name="spinner" size="sm" className="animate-spin" />
                              : <Icon name="trash" size="sm" />}
                          </button>
                        </div>
                      </div>
                      );
                    })}
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
        {lib.results.length > 5 && (
          <button
            type="button"
            onClick={() => setCollapsed(c => !c)}
            className="text-xs text-primary hover:underline w-full text-center py-1"
          >
            {collapsed
              ? t("downloads.showMore", { count: lib.results.length - 5 })
              : t("downloads.showLess")}
          </button>
        )}
      </CardContent>
    </Card>
  );
}

"use client";

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { TorrentDownloadDto } from "@/lib/api";
import { Card, CardContent, Button, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n/fr";

type TFunction = (key: TranslationKey, vars?: Record<string, string | number>) => string;

const STATUS_ACTIVE = new Set(["downloading", "completed", "importing"]);

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
}

export function DownloadsPage({ initialDownloads }: DownloadsPageProps) {
  const { t } = useTranslation();
  const [downloads, setDownloads] = useState<TorrentDownloadDto[]>(initialDownloads);
  const [filter, setFilter] = useState<string>("all");
  const [isRefreshing, setIsRefreshing] = useState(false);

  const refresh = useCallback(async (showSpinner = true) => {
    if (showSpinner) setIsRefreshing(true);
    try {
      const resp = await fetch("/api/torrent-downloads");
      if (resp.ok) setDownloads(await resp.json());
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

  return (
    <>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="download" size="xl" />
          {t("downloads.title")}
        </h1>
        <Button onClick={() => refresh(true)} disabled={isRefreshing} variant="outline" size="sm">
          {isRefreshing ? (
            <Icon name="spinner" size="sm" className="animate-spin" />
          ) : (
            <Icon name="refresh" size="sm" />
          )}
          <span className="ml-2">{t("downloads.refresh")}</span>
        </Button>
      </div>

      {/* Filter bar */}
      <div className="flex gap-1 mb-4 border-b border-border">
        {filters.map(f => (
          <button
            key={f.id}
            onClick={() => setFilter(f.id)}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors -mb-px ${
              filter === f.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
            }`}
          >
            {f.label}
            {f.id !== "all" && (
              <span className="ml-1.5 text-xs opacity-60">
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
        <div className="space-y-3">
          {visible.map(dl => (
            <DownloadCard key={dl.id} dl={dl} onDeleted={() => refresh(false)} />
          ))}
        </div>
      )}
    </>
  );
}

function DownloadCard({ dl, onDeleted }: { dl: TorrentDownloadDto; onDeleted: () => void }) {
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

  return (
    <Card>
      <CardContent className="pt-4">
        <div className="flex items-start gap-4">
          {/* Status indicator */}
          <div className="mt-0.5">
            {dl.status === "importing" ? (
              <Icon name="spinner" size="md" className="animate-spin text-primary" />
            ) : dl.status === "imported" ? (
              <Icon name="check" size="md" className="text-success" />
            ) : dl.status === "error" ? (
              <Icon name="warning" size="md" className="text-destructive" />
            ) : dl.status === "downloading" ? (
              <Icon name="download" size="md" className="text-primary" />
            ) : (
              <Icon name="refresh" size="md" className="text-warning" />
            )}
          </div>

          {/* Main info */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="font-semibold text-foreground truncate">{dl.series_name}</span>
              <span className={`text-xs font-medium px-2 py-0.5 rounded-full ${statusClass(dl.status)}`}>
                {statusLabel(dl.status, t)}
              </span>
            </div>

            <div className="mt-1 flex items-center gap-4 text-sm text-muted-foreground flex-wrap">
              {dl.expected_volumes.length > 0 && (
                <span>{t("downloads.volumes")} : {formatVolumes(dl.expected_volumes)}</span>
              )}
              {dl.status === "imported" && importedCount > 0 && (
                <span className="text-success">{importedCount} {t("downloads.filesImported")}</span>
              )}
              {dl.qb_hash && (
                <span className="font-mono text-xs opacity-50" title={dl.qb_hash}>
                  {dl.qb_hash.slice(0, 8)}…
                </span>
              )}
            </div>

            {dl.status === "downloading" && (
              <div className="mt-2">
                <div className="flex items-center gap-2 mb-1">
                  <div className="flex-1 h-2 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-full bg-primary rounded-full transition-all duration-500"
                      style={{ width: `${Math.round(dl.progress * 100)}%` }}
                    />
                  </div>
                  <span className="text-xs font-medium text-foreground tabular-nums w-10 text-right">
                    {Math.round(dl.progress * 100)}%
                  </span>
                </div>
                {(dl.download_speed > 0 || dl.eta > 0) && (
                  <div className="flex items-center gap-3 text-xs text-muted-foreground">
                    {dl.download_speed > 0 && <span>{formatSpeed(dl.download_speed)}</span>}
                    {dl.eta > 0 && dl.eta < 8640000 && <span>ETA {formatEta(dl.eta)}</span>}
                  </div>
                )}
              </div>
            )}

            {dl.content_path && dl.status !== "imported" && (
              <p className="mt-1 text-xs font-mono text-muted-foreground truncate" title={dl.content_path}>
                {dl.content_path}
              </p>
            )}

            {dl.error_message && (
              <p className="mt-1 text-sm text-destructive">{dl.error_message}</p>
            )}

            {dl.status === "imported" && Array.isArray(dl.imported_files) && dl.imported_files.length > 0 && (
              <ul className="mt-2 space-y-0.5">
                {(dl.imported_files as Array<{ volume: number; destination: string }>).map((f, i) => (
                  <li key={i} className="text-xs text-muted-foreground font-mono truncate" title={f.destination}>
                    T{String(f.volume).padStart(2, "0")} → {f.destination.split("/").pop()}
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* Actions */}
          <div className="flex flex-col items-end gap-2 shrink-0">
            <div className="text-xs text-muted-foreground text-right">
              <p>{formatDate(dl.created_at)}</p>
              {dl.updated_at !== dl.created_at && (
                <p className="opacity-60">maj {formatDate(dl.updated_at)}</p>
              )}
            </div>
            <button
              type="button"
              onClick={() => setShowConfirm(true)}
              disabled={deleting || dl.status === "importing"}
              className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30"
              title={dl.status === "downloading" ? t("downloads.cancel") : t("downloads.delete")}
            >
              {deleting ? (
                <Icon name="spinner" size="sm" className="animate-spin" />
              ) : (
                <Icon name="trash" size="sm" />
              )}
            </button>
          </div>
        </div>
      </CardContent>

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
    </Card>
  );
}

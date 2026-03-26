"use client";

import { useState, useEffect, useCallback } from "react";
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
    const id = setInterval(() => refresh(false), 5000);
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
            <DownloadCard key={dl.id} dl={dl} />
          ))}
        </div>
      )}
    </>
  );
}

function DownloadCard({ dl }: { dl: TorrentDownloadDto }) {
  const { t } = useTranslation();
  const importedCount = Array.isArray(dl.imported_files) ? dl.imported_files.length : 0;

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

          {/* Timestamp */}
          <div className="text-xs text-muted-foreground shrink-0 text-right">
            <p>{formatDate(dl.created_at)}</p>
            {dl.updated_at !== dl.created_at && (
              <p className="opacity-60">maj {formatDate(dl.updated_at)}</p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

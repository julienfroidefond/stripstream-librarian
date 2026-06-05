"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import { useTranslation } from "../../lib/i18n/context";
import { ProgressBar } from "./ui/ProgressBar";

interface Download {
  id: string;
  source: "prowlarr" | "telegram";
  library_id: string;
  series_id?: string;
  series_name: string;
  expected_volumes: number[];
  status: "downloading" | "completed" | "importing" | "imported" | "error";
  progress: number;
  download_speed: number;
  eta: number;
  error_message: string | null;
}

interface TelegramDownload {
  id: string;
  series_name: string | null;
  series_id: string | null;
  library_id: string | null;
  filename: string;
  file_size: number | null;
  bytes_downloaded: number;
  volume_number: number | null;
  status: "downloading" | "imported" | "failed";
  error_message: string | null;
}

const STATUS_ACTIVE = new Set(["downloading", "completed", "importing"]);

function normalizeTorrentDownload(download: Omit<Download, "source">): Download {
  return {
    ...download,
    id: `torrent-${download.id}`,
    source: "prowlarr",
  };
}

function normalizeTelegramDownload(download: TelegramDownload): Download {
  const progress = download.file_size && download.file_size > 0
    ? Math.min(1, download.bytes_downloaded / download.file_size)
    : 0;

  return {
    id: `telegram-${download.id}`,
    source: "telegram",
    library_id: download.library_id ?? "",
    series_id: download.series_id ?? undefined,
    series_name: download.series_name ?? download.filename,
    expected_volumes: download.volume_number != null ? [download.volume_number] : [],
    status: download.status === "failed" ? "error" : download.status,
    progress,
    download_speed: 0,
    eta: 0,
    error_message: download.error_message,
  };
}

// Icons
const DownloadIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" strokeLinecap="round" strokeLinejoin="round" />
    <polyline points="7 10 12 15 17 10" strokeLinecap="round" strokeLinejoin="round" />
    <line x1="12" y1="15" x2="12" y2="3" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const SpinnerIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
    <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
    <path d="M12 2a10 10 0 0 1 10 10" strokeLinecap="round" />
  </svg>
);

const ChevronIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M6 9l6 6 6-6" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

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

export function DownloadsIndicator() {
  const { t } = useTranslation();
  const [activeTorrentDownloads, setActiveTorrentDownloads] = useState<Download[]>([]);
  const [activeTelegramDownloads, setActiveTelegramDownloads] = useState<Download[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popinRef = useRef<HTMLDivElement>(null);
  const [popinStyle, setPopinStyle] = useState<React.CSSProperties>({});

  useEffect(() => {
    let eventSource: EventSource | null = null;
    let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    let staleTimeout: ReturnType<typeof setTimeout> | null = null;

    const resetStaleTimer = () => {
      if (staleTimeout) clearTimeout(staleTimeout);
      staleTimeout = setTimeout(() => {
        eventSource?.close();
        eventSource = null;
        connect();
      }, 30000);
    };

    const connect = () => {
      if (eventSource) {
        eventSource.close();
      }
      eventSource = new EventSource("/api/torrent-downloads/stream");
      resetStaleTimer();

      eventSource.onmessage = (event) => {
        resetStaleTimer();
        try {
          const allDownloads: Omit<Download, "source">[] = JSON.parse(event.data);
          const active = allDownloads
            .filter(d => STATUS_ACTIVE.has(d.status))
            .map(normalizeTorrentDownload);
          setActiveTorrentDownloads(active);
        } catch {
          // ignore malformed data
        }
      };

      eventSource.onerror = () => {
        eventSource?.close();
        eventSource = null;
        reconnectTimeout = setTimeout(connect, 3000);
      };
    };

    const disconnect = () => {
      if (reconnectTimeout) { clearTimeout(reconnectTimeout); reconnectTimeout = null; }
      if (staleTimeout) { clearTimeout(staleTimeout); staleTimeout = null; }
      if (eventSource) { eventSource.close(); eventSource = null; }
    };

    const handleVisibilityChange = () => {
      if (document.hidden) {
        disconnect();
      } else {
        connect();
      }
    };

    connect();
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      disconnect();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    let interval: ReturnType<typeof setInterval> | null = null;
    let abortController: AbortController | null = null;

    const fetchTelegramDownloads = async () => {
      abortController?.abort();
      abortController = new AbortController();

      try {
        const resp = await fetch("/api/telegram-monitor/downloads", {
          signal: abortController.signal,
        });
        if (!resp.ok) return;
        const allDownloads: TelegramDownload[] = await resp.json();
        setActiveTelegramDownloads(
          allDownloads
            .filter(download => download.status === "downloading")
            .map(normalizeTelegramDownload)
        );
      } catch (error) {
        if ((error as Error).name !== "AbortError") {
          // Keep the last known state on transient failures.
        }
      }
    };

    const start = () => {
      fetchTelegramDownloads();
      interval = setInterval(fetchTelegramDownloads, 5000);
    };

    const stop = () => {
      if (interval) { clearInterval(interval); interval = null; }
      abortController?.abort();
      abortController = null;
    };

    const handleVisibilityChange = () => {
      if (document.hidden) {
        stop();
      } else {
        start();
      }
    };

    start();
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      stop();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  // Position the popin relative to the button
  const updatePosition = useCallback(() => {
    if (!buttonRef.current) return;
    const rect = buttonRef.current.getBoundingClientRect();
    const isMobile = window.innerWidth < 640;

    if (isMobile) {
      setPopinStyle({
        position: "fixed",
        top: `${rect.bottom + 8}px`,
        left: "12px",
        right: "12px",
      });
    } else {
      const rightEdge = window.innerWidth - rect.right;
      setPopinStyle({
        position: "fixed",
        top: `${rect.bottom + 8}px`,
        right: `${Math.max(rightEdge, 12)}px`,
        width: "384px",
      });
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [isOpen, updatePosition]);

  // Close when clicking outside
  useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        buttonRef.current && !buttonRef.current.contains(target) &&
        popinRef.current && !popinRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [isOpen]);

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return;
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("keydown", handleEsc);
    return () => document.removeEventListener("keydown", handleEsc);
  }, [isOpen]);

  const activeDownloads = [...activeTorrentDownloads, ...activeTelegramDownloads];
  const downloadingItems = activeDownloads.filter(d => d.status === "downloading");
  const importingItems = activeDownloads.filter(d => d.status === "importing" || d.status === "completed");
  const totalCount = activeDownloads.length;

  const totalProgress = downloadingItems.length > 0
    ? downloadingItems.reduce((acc, d) => acc + d.progress, 0) / downloadingItems.length
    : 0;

  if (totalCount === 0) {
    return (
      <Link
        href="/downloads"
        className="
          flex items-center justify-center
          h-9 w-9
          rounded-md
          text-muted-foreground
          hover:text-foreground
          hover:bg-accent
          transition-colors duration-200
        "
        title={t("downloadsIndicator.viewAll")}
      >
        <DownloadIcon className="w-3.5 h-3.5" />
      </Link>
    );
  }

  const popin = isOpen && (
    <>
      {/* Mobile backdrop */}
      <div
        className="fixed inset-0 z-[80] sm:hidden bg-background/60 backdrop-blur-sm"
        onClick={() => setIsOpen(false)}
        aria-hidden="true"
      />

      {/* Popin */}
      <div
        ref={popinRef}
        style={popinStyle}
        className="
          z-[90]
          bg-popover/95 backdrop-blur-md
          rounded-xl
          shadow-elevation-2
          border border-border/60
          overflow-hidden
          animate-fade-in
        "
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border/60 bg-muted/50">
          <div className="flex items-center gap-3">
            <span className="text-xl">⬇️</span>
            <div>
              <h3 className="font-semibold text-foreground">{t("downloadsIndicator.activeDownloads")}</h3>
              <p className="text-xs text-muted-foreground">
                {downloadingItems.length > 0 && importingItems.length > 0
                  ? t("downloadsIndicator.downloadingAndImporting", { downloading: downloadingItems.length, importing: importingItems.length })
                  : downloadingItems.length > 0
                    ? t("downloadsIndicator.downloadingCount", { count: downloadingItems.length, plural: downloadingItems.length !== 1 ? "s" : "" })
                    : t("downloadsIndicator.importingCount", { count: importingItems.length, plural: importingItems.length !== 1 ? "s" : "" })
                }
              </p>
            </div>
          </div>
          <Link
            href="/downloads"
            className="text-sm font-medium text-primary hover:text-primary/80 transition-colors"
            onClick={() => setIsOpen(false)}
          >
            {t("downloadsIndicator.viewAllLink")}
          </Link>
        </div>

        {/* Overall progress bar if downloading */}
        {downloadingItems.length > 0 && (
          <div className="px-4 py-3 border-b border-border/60">
            <div className="flex items-center justify-between text-sm mb-2">
              <span className="text-muted-foreground">{t("downloadsIndicator.overallProgress")}</span>
              <span className="font-semibold text-foreground">{Math.round(totalProgress * 100)}%</span>
            </div>
            <ProgressBar value={totalProgress * 100} size="sm" variant="default" />
          </div>
        )}

        {/* Download List */}
        <div className="max-h-80 overflow-y-auto scrollbar-hide">
          <ul className="divide-y divide-border/60">
            {activeDownloads.map(dl => (
              <li key={dl.id}>
                <Link
                  href="/downloads"
                  className="block px-4 py-3 hover:bg-accent/50 transition-colors duration-200"
                  onClick={() => setIsOpen(false)}
                >
                  <div className="flex items-start gap-3">
                    <div className="mt-0.5">
                      {dl.status === "downloading" && <span className="animate-pulse inline-block">⬇️</span>}
                      {dl.status === "importing" && <span className="animate-spin inline-block">⏳</span>}
                      {dl.status === "completed" && <span>✅</span>}
                    </div>

                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="text-sm font-medium text-foreground truncate">{dl.series_name}</span>
                        {dl.source === "telegram" && (
                          <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-sky-500/10 text-sky-600">
                            Telegram
                          </span>
                        )}
                        <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full ${statusClass(dl.status)}`}>
                          {statusLabel(dl.status, t)}
                        </span>
                      </div>

                      {dl.expected_volumes.length > 0 && (
                        <p className="text-[11px] text-muted-foreground mb-1">
                          {formatVolumes(dl.expected_volumes)}
                        </p>
                      )}

                      {dl.status === "downloading" && (
                        <div className="flex items-center gap-2 mt-1">
                          <MiniProgressBar value={dl.progress * 100} />
                          <span className="text-xs font-medium text-muted-foreground tabular-nums">
                            {Math.round(dl.progress * 100)}%
                          </span>
                          {dl.download_speed > 0 && (
                            <span className="text-[10px] text-muted-foreground">{formatSpeed(dl.download_speed)}</span>
                          )}
                          {dl.eta > 0 && dl.eta < 8640000 && (
                            <span className="text-[10px] text-muted-foreground">ETA {formatEta(dl.eta)}</span>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                </Link>
              </li>
            ))}
          </ul>
        </div>

        {/* Footer */}
        <div className="px-4 py-2 border-t border-border/60 bg-muted/50">
          <p className="text-xs text-muted-foreground text-center">{t("downloadsIndicator.autoRefresh")}</p>
        </div>
      </div>
    </>
  );

  return (
    <>
      <button
        ref={buttonRef}
        className={`
          flex items-center gap-1.5
          h-9 px-2.5
          rounded-lg
          border
          text-xs font-medium
          transition-all duration-200
          ${downloadingItems.length > 0
            ? 'border-primary/40 bg-primary/10 text-primary hover:bg-primary/15'
            : 'border-warning/40 bg-warning/10 text-warning hover:bg-warning/15'
          }
          ${isOpen ? 'ring-2 ring-ring ring-offset-2 ring-offset-background' : ''}
        `}
        onClick={() => setIsOpen(!isOpen)}
        title={t("downloadsIndicator.downloadCount", { count: totalCount, plural: totalCount !== 1 ? "s" : "" })}
      >
        {downloadingItems.length > 0 && (
          <div className="w-3.5 h-3.5 animate-spin">
            <SpinnerIcon className="w-3.5 h-3.5" />
          </div>
        )}

        <DownloadIcon className="w-3.5 h-3.5" />

        <span className="flex items-center justify-center min-w-4 h-4 px-1 text-[10px] font-bold bg-current rounded-full">
          <span className="text-background">{totalCount > 99 ? "99+" : totalCount}</span>
        </span>

        <ChevronIcon
          className={`w-3 h-3 hidden sm:block transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {typeof document !== "undefined" && createPortal(popin, document.body)}
    </>
  );
}

function statusClass(status: string): string {
  switch (status) {
    case "downloading": return "bg-primary/10 text-primary";
    case "completed":   return "bg-warning/10 text-warning";
    case "importing":   return "bg-primary/10 text-primary";
    default:            return "bg-muted/30 text-muted-foreground";
  }
}

function statusLabel(status: string, t: (key: any, vars?: Record<string, string | number>) => string): string {
  const map: Record<string, string> = {
    downloading: "downloads.status.downloading",
    completed:   "downloads.status.completed",
    importing:   "downloads.status.importing",
  };
  return t(map[status] ?? status);
}

function formatVolumes(vols: number[]): string {
  return [...vols].sort((a, b) => a - b).map(v => `T${String(v).padStart(2, "0")}`).join(", ");
}

// Mini progress bar for dropdown
function MiniProgressBar({ value }: { value: number }) {
  return (
    <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
      <div
        className="h-full bg-primary rounded-full transition-all duration-300"
        style={{ width: `${value}%` }}
      />
    </div>
  );
}

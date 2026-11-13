"use client";

import { useState, useCallback, useEffect } from "react";
import Link from "next/link";
import {
  TorrentDownloadDto,
  LatestFoundPerLibraryDto,
  TelegramAvailableGroupDto,
  TelegramDownloadItemDto,
  DdlProviderDownloadDto,
  DownloadClient,
} from "@/lib/api";
import {
  availableLibraries,
  availableSourceVolumes,
  buildAvailableGroups,
  downloadStatusClass,
  downloadStatusLabel,
  filterAvailableGroups,
  matchesDownloadFilter,
  mergeDownloads,
  sortAvailableGroups,
  summarizeAvailableSources,
  useDownloadsStream,
  visibleAvailableSources,
  type AvailableGroup,
  type AvailableSortKey,
  type AvailableSource,
  type AvailableSourceFilter,
  type DownloadFilter,
} from "@/lib/downloads";
import { Card, CardContent, CardHeader, CardTitle, Button, Icon } from "@/app/components/ui";
import { browserApi } from "@/lib/api/browser";
import type { DownloadAction } from "@/lib/downloadActions";
import { DownloadClientProvider, DownloadClientButton, DownloadClientIndicator, DownloadClientBadge } from "@/app/components/DownloadClientButton";
import { DownloadActionsProvider, useDownloadActions } from "./useDownloadActions";
import { useTranslation } from "@/lib/i18n/context";
import { compressVolumes } from "@/lib/volumeRanges";
import { formatDateTime, formatEta, formatSpeed, formatVolumes } from "@/lib/format";
import { isTelegramDownloadRetryable, showsDownloadProgress } from "@/lib/download";

interface DownloadsPageProps {
  initialDownloads: TorrentDownloadDto[];
  initialLatestFound: LatestFoundPerLibraryDto[];
  qbConfigured?: boolean;
  alldebridConfigured?: boolean;
  /** Client chosen in Settings; read server-side to avoid a first-render flash. */
  initialDownloadClient?: DownloadClient | null;
  initialTelegramAvailable?: TelegramAvailableGroupDto[];
  initialTelegramDownloads?: TelegramDownloadItemDto[];
  initialDdlProviderDownloads?: DdlProviderDownloadDto[];
}

const PAGE_SIZE = 10;

export function DownloadsPage({ initialDownloads, initialLatestFound, qbConfigured, alldebridConfigured, initialDownloadClient = null, initialTelegramAvailable = [], initialTelegramDownloads = [], initialDdlProviderDownloads = [] }: DownloadsPageProps) {
  const { t } = useTranslation();
  // Progress fields are pushed by the shared SSE stream. Keep the full refresh
  // for manual actions and the slower-changing availability sections only.
  const {
    torrent: downloads,
    telegram: telegramDownloads,
    ddl: ddlProviderDownloads,
    setTorrent: setDownloads,
    setTelegram: setTelegramDownloads,
    setDdl: setDdlProviderDownloads,
  } = useDownloadsStream({
    torrent: initialDownloads,
    telegram: initialTelegramDownloads,
    ddl: initialDdlProviderDownloads,
  });
  const [latestFound, setLatestFound] = useState<LatestFoundPerLibraryDto[]>(initialLatestFound);
  const [telegramAvailable, setTelegramAvailable] = useState<TelegramAvailableGroupDto[]>(initialTelegramAvailable);
  const [statusFilter, setStatusFilter] = useState<DownloadFilter>("all");
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [page, setPage] = useState(1);

  const refresh = useCallback(async (showSpinner = true) => {
    if (showSpinner) setIsRefreshing(true);
    try {
      const [dlResp, lfResp, tgResp, tgDlResp, provResp] = await Promise.allSettled([
        browserApi.downloads.torrents.list(),
        browserApi.downloads.detection.latest(),
        browserApi.telegram.available(),
        browserApi.telegram.downloads(),
        browserApi.ddl.downloads(),
      ]);
      if (dlResp.status === "fulfilled") setDownloads(dlResp.value);
      if (lfResp.status === "fulfilled") setLatestFound(lfResp.value);
      if (tgResp.status === "fulfilled") setTelegramAvailable(tgResp.value);
      if (tgDlResp.status === "fulfilled") setTelegramDownloads(tgDlResp.value);
      if (provResp.status === "fulfilled") setDdlProviderDownloads(provResp.value);
    } finally {
      if (showSpinner) setIsRefreshing(false);
    }
  }, [setDownloads, setTelegramDownloads, setDdlProviderDownloads]);

  /**
   * What a successful write refreshes. Starting a Telegram download only flips
   * that book to "queued" locally: the available endpoint stops listing it as
   * soon as its status changes, and the SSE stream pushes the new download row.
   */
  const handleActionSuccess = useCallback((action: DownloadAction, id: string) => {
    if (action.target === "telegram" && action.verb === "start") {
      setTelegramAvailable(prev => prev.map(group => ({
        ...group,
        books: group.books.map(book => book.id === id ? { ...book, status: "queued" } : book),
      })));
      return;
    }
    void refresh(false);
  }, [refresh]);

  const merged = mergeDownloads(downloads, telegramDownloads, ddlProviderDownloads);

  const statusFilters: Array<{ id: DownloadFilter; label: string }> = [
    { id: "all", label: t("common.all") },
    { id: "active", label: t("downloads.filterActive") },
    { id: "imported", label: t("downloads.status.imported") },
    { id: "error", label: t("downloads.status.error") },
  ];

  const visible = merged.filter(item => matchesDownloadFilter(item, statusFilter));
  const totalPages = Math.ceil(visible.length / PAGE_SIZE);
  const paged = visible.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  // Reset to page 1 when filters change
  const handleStatusFilterChange = (id: DownloadFilter) => { setStatusFilter(id); setPage(1); };

  return (
    <DownloadClientProvider initialClients={{ qbittorrent: qbConfigured, alldebrid: alldebridConfigured }} initialClient={initialDownloadClient} onDownloadStarted={() => refresh(false)}>
      <DownloadActionsProvider onRefresh={handleActionSuccess}>
        <div className="flex items-center justify-between mb-4 sm:mb-6 gap-2">
          <h1 className="text-xl sm:text-3xl font-bold text-foreground flex items-center gap-2 sm:gap-3">
            <Icon name="download" size="lg" className="sm:hidden text-success" />
            <Icon name="download" size="xl" className="hidden sm:block text-success" />
            {t("downloads.title")}
          </h1>
          <div className="flex items-center gap-2 sm:gap-3">
            <DownloadClientIndicator />
            <Button onClick={() => refresh(true)} disabled={isRefreshing} variant="outline" size="sm">
              {isRefreshing ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="refresh" size="sm" />}
              <span className="ml-2 hidden sm:inline">{t("downloads.refresh")}</span>
            </Button>
          </div>
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
                <span className="ml-1 sm:ml-1.5 text-3xs sm:text-xs opacity-60">
                  {merged.filter(item => matchesDownloadFilter(item, f.id)).length}
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
                ? <DownloadRow key={item.key} dl={item.data} />
                : item.kind === "telegram"
                ? <TelegramDownloadRow key={item.key} item={item.data} />
                : <DdlProviderDownloadRow key={item.key} item={item.data} />
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
          <AvailableDownloadsSection
            latestFound={latestFound}
            telegramAvailable={telegramAvailable}
          />
        )}
      </DownloadActionsProvider>
    </DownloadClientProvider>
  );
}

const RETRYABLE_STATUSES = new Set(["importing", "error", "no_files_imported", "partial"]);

export function DownloadRow({ dl }: { dl: TorrentDownloadDto }) {
  const { t } = useTranslation();
  const { busy, state, run } = useDownloadActions();
  const actionState = state(dl.id);
  const pendingVerb = actionState.status === "pending" ? actionState.verb : null;
  const retrying = pendingVerb === "retry";
  const deleting = pendingVerb === "delete";
  const allImported = Array.isArray(dl.imported_files) ? dl.imported_files : [];
  const importedCount = allImported.filter((f: { already_existed?: boolean }) => !f.already_existed).length;
  const alreadyExistedCount = allImported.filter((f: { already_existed?: boolean }) => f.already_existed).length;

  function handleDelete() {
    const cancelling = dl.status === "downloading";
    run({ target: "torrent", verb: "delete" }, dl.id, {
      confirm: {
        title: cancelling ? t("downloads.cancel") : t("downloads.delete"),
        message: cancelling ? t("downloads.confirmCancel") : t("downloads.confirmDelete"),
        confirmLabel: cancelling ? t("downloads.cancel") : t("downloads.delete"),
      },
    });
  }

  function handleRetry() {
    // A failed AllDebrid magnet never produced local files: restart the magnet
    // itself instead of the (impossible) import. Import errors on a downloaded
    // torrent keep going through the regular import retry.
    const magnetFailed = dl.client === "alldebrid" && dl.status === "error" && !dl.content_path;
    run(magnetFailed
      ? { target: "alldebrid", verb: "retry" }
      : { target: "torrent", verb: "retry" }, dl.id);
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
            <span className={`text-3xs font-medium px-1.5 py-0.5 rounded-full ${downloadStatusClass(dl.status)}`}>
              {downloadStatusLabel(dl.status, t)}
            </span>
            <DownloadClientBadge client={dl.client} />
            {dl.expected_volumes.length > 0 && (
              <span className="text-2xs text-muted-foreground">{formatVolumes(dl.expected_volumes)}</span>
            )}
            {dl.status === "imported" && importedCount > 0 && (
              <span className="text-2xs text-success">{importedCount} {t("downloads.filesImported")}</span>
            )}
            {dl.status === "imported" && importedCount === 0 && alreadyExistedCount > 0 && (
              <span className="text-2xs text-muted-foreground">{alreadyExistedCount} {t("downloads.alreadyExisted")}</span>
            )}
            {dl.status === "imported" && importedCount > 0 && alreadyExistedCount > 0 && (
              <span className="text-2xs text-muted-foreground">({alreadyExistedCount} {t("downloads.alreadyExisted")})</span>
            )}
          </div>

          {/* Mobile: stacked */}
          <div className="sm:hidden">
            <div className="flex items-center gap-1.5">
              <Link href={`/series/${dl.series_id ?? encodeURIComponent(dl.series_name)}`} className="text-sm font-medium text-primary hover:underline truncate">{dl.series_name}</Link>
              <span className={`text-3xs font-medium px-1.5 py-0.5 rounded-full shrink-0 ${downloadStatusClass(dl.status)}`}>
                {downloadStatusLabel(dl.status, t)}
              </span>
              <DownloadClientBadge client={dl.client} />
            </div>
            <div className="flex items-center gap-2 mt-0.5 text-2xs text-muted-foreground">
              {dl.expected_volumes.length > 0 && <span>{formatVolumes(dl.expected_volumes)}</span>}
              {dl.status === "imported" && importedCount > 0 && (
                <span className="text-success">{importedCount} {t("downloads.filesImported")}</span>
              )}
              {dl.status === "imported" && importedCount === 0 && alreadyExistedCount > 0 && (
                <span>{alreadyExistedCount} {t("downloads.alreadyExisted")}</span>
              )}
              <span className="tabular-nums">{formatDateTime(dl.created_at, "fr")}</span>
            </div>
          </div>

          {/* Torrent / release name reported by the download backend */}
          {dl.name && (
            <p className="text-2xs text-muted-foreground truncate mt-0.5" title={dl.name}>{dl.name}</p>
          )}

          {showsDownloadProgress(dl.status, dl.progress) && (
            <div className="flex items-center gap-2 mt-1">
              <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden max-w-xs">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-500"
                  style={{ width: `${Math.round(dl.progress * 100)}%` }}
                />
              </div>
              <span className="text-3xs font-medium text-foreground tabular-nums">
                {Math.round(dl.progress * 100)}%
              </span>
              {dl.file_size != null && dl.file_size > 0 && (
                <span className="text-3xs text-muted-foreground">{formatSize(dl.file_size)}</span>
              )}
              {dl.status === "downloading" && dl.download_speed > 0 && (
                <span className="text-3xs text-muted-foreground hidden sm:inline">{formatSpeed(dl.download_speed)}</span>
              )}
              {dl.status === "downloading" && dl.eta > 0 && dl.eta < 8640000 && (
                <span className="text-3xs text-muted-foreground">ETA {formatEta(dl.eta)}</span>
              )}
              {dl.status === "downloading" && dl.seeders != null && (
                <span className={`text-3xs ${dl.seeders === 0 ? "text-warning" : "text-muted-foreground"}`}>
                  {t("downloads.seeders", { count: String(dl.seeders), plural: dl.seeders !== 1 ? "s" : "" })}
                </span>
              )}
            </div>
          )}

          {dl.error_message && (
            <p className="text-2xs text-destructive truncate mt-0.5" title={dl.error_message}>{dl.error_message}</p>
          )}
        </div>

        <span className="text-3xs text-muted-foreground shrink-0 tabular-nums hidden sm:block">{formatDateTime(dl.created_at, "fr")}</span>

        {canRetry && (
          <button
            type="button"
            onClick={handleRetry}
            disabled={busy(dl.id)}
            className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors disabled:opacity-30 shrink-0"
            title={t("downloads.retry")}
          >
            {retrying ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="refresh" size="sm" />}
          </button>
        )}
        <button
          type="button"
          onClick={handleDelete}
          disabled={busy(dl.id) || dl.status === "importing"}
          className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
          title={dl.status === "downloading" ? t("downloads.cancel") : t("downloads.delete")}
        >
          {deleting ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="trash" size="sm" />}
        </button>
      </div>
    </>
  );
}

/** True when a download has a rate or an ETA worth showing. */
function hasTransferStats(speed: number, eta: number): boolean {
  return speed > 0 || formatEta(eta) !== "";
}

/** Speed + ETA fragment shared by the Telegram and DDL download rows. */
function TransferStats({ speed, eta }: { speed: number; eta: number }) {
  const etaLabel = formatEta(eta);
  return (
    <>
      {speed > 0 && <>{formatSpeed(speed)}{etaLabel && " · "}</>}
      {etaLabel && <>ETA {etaLabel}</>}
    </>
  );
}

function TelegramDownloadRow({ item }: { item: TelegramDownloadItemDto }) {
  const { t } = useTranslation();
  const { busy, state, run } = useDownloadActions();
  const actionState = state(item.id);
  const pendingVerb = actionState.status === "pending" ? actionState.verb : null;
  const retrying = pendingVerb === "retry";
  const deleting = pendingVerb === "dismiss" || pendingVerb === "delete";

  function handleRetry() {
    run({ target: "telegram", verb: "retry" }, item.id);
  }

  // The trash hides the row and blacklists the release: it stays reversible from
  // the hidden-releases panel, but a running transfer is cancelled on the spot.
  function handleDelete() {
    const cancelling = item.status === "queued" || item.status === "downloading";
    run({ target: "telegram", verb: "dismiss" }, item.id, {
      confirm: {
        title: cancelling ? t("downloads.cancel") : t("downloads.delete"),
        message: item.status === "downloading" ? t("downloads.confirmCancel") : t("downloads.confirmDelete"),
        confirmLabel: cancelling ? t("downloads.cancel") : t("downloads.delete"),
      },
    });
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
            <span className={`text-3xs font-medium px-1.5 py-0.5 rounded-full ${downloadStatusClass(tgStatus)}`}>
              {downloadStatusLabel(tgStatus, t)}
            </span>
            {volLabel && <span className="text-2xs text-muted-foreground">{volLabel}</span>}
            <span className="text-2xs text-muted-foreground truncate">@{item.channel_username}</span>
          </div>
          {/* Mobile */}
          <div className="sm:hidden">
            <div className="flex items-center gap-1.5">
              {item.series_id
                ? <Link href={`/series/${item.series_id}`} className="text-sm font-medium text-primary hover:underline truncate">{item.series_name ?? item.filename}</Link>
                : <span className="text-sm font-medium text-foreground truncate">{item.series_name ?? item.filename}</span>
              }
              <span className={`text-3xs font-medium px-1.5 py-0.5 rounded-full shrink-0 ${downloadStatusClass(tgStatus)}`}>
                {downloadStatusLabel(tgStatus, t)}
              </span>
            </div>
            <div className="flex items-center gap-2 mt-0.5 text-2xs text-muted-foreground">
              {volLabel && <span>{volLabel}</span>}
              {item.file_size != null && item.file_size > 0 && <span>{formatSize(item.file_size)}</span>}
              <span className="tabular-nums">{formatDateTime(item.updated_at, "fr")}</span>
            </div>
          </div>
          {/* Filename secondary */}
          <p className="hidden sm:block text-2xs text-muted-foreground truncate mt-0.5">
            {item.filename}
            {item.status !== "downloading" && item.file_size != null && item.file_size > 0 && (
              <> · {formatSize(item.file_size)}</>
            )}
          </p>
          {/* Progress bar for active downloads */}
          {item.status === "downloading" && item.file_size && item.file_size > 0 && (
            <div className="mt-1.5 space-y-0.5">
              <div className="w-full bg-muted/50 rounded-full h-1.5 overflow-hidden">
                <div
                  className="bg-primary h-1.5 rounded-full transition-all duration-500"
                  style={{ width: `${Math.min(100, Math.round((item.bytes_downloaded / item.file_size) * 100))}%` }}
                />
              </div>
              <p className="text-3xs text-muted-foreground tabular-nums">
                {formatSize(item.bytes_downloaded)} / {formatSize(item.file_size)}
                {" · "}{Math.min(100, Math.round((item.bytes_downloaded / item.file_size) * 100))}%
                {hasTransferStats(item.download_speed, item.eta) && (
                  <>{" · "}<TransferStats speed={item.download_speed} eta={item.eta} /></>
                )}
              </p>
            </div>
          )}
          {item.status === "downloading" && (!item.file_size || item.file_size <= 0) && hasTransferStats(item.download_speed, item.eta) && (
            <p className="mt-1 text-3xs text-muted-foreground tabular-nums">
              <TransferStats speed={item.download_speed} eta={item.eta} />
            </p>
          )}
          {item.error_message && (
            <p className="text-2xs text-destructive truncate mt-0.5" title={item.error_message}>{item.error_message}</p>
          )}
        </div>

        <span className="text-3xs text-muted-foreground shrink-0 tabular-nums hidden sm:block">{formatDateTime(item.updated_at, "fr")}</span>

        {isTelegramDownloadRetryable(item.status) && (
          <button
            type="button"
            onClick={handleRetry}
            disabled={busy(item.id)}
            className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors disabled:opacity-30 shrink-0"
            title={t("downloads.retry")}
          >
            {retrying ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="refresh" size="sm" />}
          </button>
        )}
        <button
          type="button"
          onClick={handleDelete}
          disabled={busy(item.id)}
          className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
          title={(item.status === "queued" || item.status === "downloading") ? t("downloads.cancel") : t("downloads.delete")}
        >
          {deleting ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="trash" size="sm" />}
        </button>
      </div>
    </>
  );
}

function DdlProviderDownloadRow({ item }: { item: DdlProviderDownloadDto }) {
  const { t } = useTranslation();
  const { busy, state, run } = useDownloadActions();
  const actionState = state(item.id);
  const pendingVerb = actionState.status === "pending" ? actionState.verb : null;
  const retrying = pendingVerb === "retry";
  const deleting = pendingVerb === "dismiss" || pendingVerb === "delete";

  const isActive = item.status === "queued" || item.status === "downloading";
  const canRetry = !!item.library_id && (item.status === "failed" || item.status === "imported" || item.status === "dismissed");

  function handleRetry() {
    run({ target: "ddl", verb: "retry" }, item.id, { context: { ddlDownload: item } });
  }

  function handleDelete() {
    // Active downloads are only dismissed (aborted + hidden); finished ones are removed.
    run({ target: "ddl", verb: isActive ? "dismiss" : "delete" }, item.id, {
      confirm: {
        title: isActive ? t("downloads.cancel") : t("downloads.delete"),
        message: item.status === "downloading" ? t("downloads.confirmCancel") : t("downloads.confirmDelete"),
        confirmLabel: isActive ? t("downloads.cancel") : t("downloads.delete"),
      },
    });
  }

  const ddlStatus = item.status === "failed" ? "error" : item.status;
  const volLabel = item.volume_number != null ? `T${String(item.volume_number).padStart(2, "0")}` : null;

  const statusIcon = item.status === "imported"
    ? <Icon name="check" size="sm" className="text-success" />
    : item.status === "downloading"
    ? <Icon name="download" size="sm" className="text-primary" />
    : item.status === "queued"
    ? <Icon name="clock" size="sm" className="text-muted-foreground" />
    : <Icon name="warning" size="sm" className="text-destructive" />;

  const title = item.series_name ?? item.filename;

  return (
    <>
      <div className="flex items-start sm:items-center gap-2 sm:gap-3 px-3 py-2 rounded-lg border border-border/40 bg-card hover:bg-accent/30 transition-colors">
        <div className="mt-0.5 sm:mt-0">{statusIcon}</div>

        <div className="flex-1 min-w-0">
          {/* Desktop */}
          <div className="hidden sm:flex items-center gap-2">
            {item.series_id
              ? <Link href={`/series/${item.series_id}`} className="text-sm font-medium text-primary hover:underline truncate">{title}</Link>
              : <span className="text-sm font-medium text-foreground truncate">{title}</span>
            }
            <span className={`text-3xs font-medium px-1.5 py-0.5 rounded-full ${downloadStatusClass(ddlStatus)}`}>
              {downloadStatusLabel(ddlStatus, t)}
            </span>
            {volLabel && <span className="text-2xs text-muted-foreground">{volLabel}</span>}
            <span className="inline-flex items-center gap-1 text-2xs text-muted-foreground">
              <Icon name="globe" size="sm" className="text-success" />
              {item.provider}
            </span>
            {item.library_name && <span className="text-2xs text-muted-foreground truncate">{item.library_name}</span>}
          </div>
          {/* Mobile */}
          <div className="sm:hidden">
            <div className="flex items-center gap-1.5">
              {item.series_id
                ? <Link href={`/series/${item.series_id}`} className="text-sm font-medium text-primary hover:underline truncate">{title}</Link>
                : <span className="text-sm font-medium text-foreground truncate">{title}</span>
              }
              <span className={`text-3xs font-medium px-1.5 py-0.5 rounded-full shrink-0 ${downloadStatusClass(ddlStatus)}`}>
                {downloadStatusLabel(ddlStatus, t)}
              </span>
            </div>
            <div className="flex items-center gap-2 mt-0.5 text-2xs text-muted-foreground">
              {volLabel && <span>{volLabel}</span>}
              <span className="inline-flex items-center gap-1"><Icon name="globe" size="sm" className="text-success" />{item.provider}</span>
              {item.file_size != null && item.file_size > 0 && <span>{formatSize(item.file_size)}</span>}
              <span className="tabular-nums">{formatDateTime(item.updated_at, "fr")}</span>
            </div>
          </div>
          {/* Filename secondary */}
          <p className="hidden sm:block text-2xs text-muted-foreground truncate mt-0.5">
            {item.filename}
            {item.status !== "downloading" && item.file_size != null && item.file_size > 0 && (
              <> · {formatSize(item.file_size)}</>
            )}
          </p>
          {/* Progress bar for active downloads */}
          {item.status === "downloading" && item.file_size && item.file_size > 0 && (
            <div className="mt-1.5 space-y-0.5">
              <div className="w-full bg-muted/50 rounded-full h-1.5 overflow-hidden">
                <div
                  className="bg-primary h-1.5 rounded-full transition-all duration-500"
                  style={{ width: `${Math.min(100, Math.round((item.bytes_downloaded / item.file_size) * 100))}%` }}
                />
              </div>
              <p className="text-3xs text-muted-foreground tabular-nums">
                {formatSize(item.bytes_downloaded)} / {formatSize(item.file_size)}
                {" · "}{Math.min(100, Math.round((item.bytes_downloaded / item.file_size) * 100))}%
                {hasTransferStats(item.download_speed, item.eta) && (
                  <>{" · "}<TransferStats speed={item.download_speed} eta={item.eta} /></>
                )}
              </p>
            </div>
          )}
          {item.status === "downloading" && (!item.file_size || item.file_size <= 0) && hasTransferStats(item.download_speed, item.eta) && (
            <p className="mt-1 text-3xs text-muted-foreground tabular-nums">
              <TransferStats speed={item.download_speed} eta={item.eta} />
            </p>
          )}
          {item.error_message && (
            <p className="text-2xs text-destructive truncate mt-0.5" title={item.error_message}>{item.error_message}</p>
          )}
        </div>

        <span className="text-3xs text-muted-foreground shrink-0 tabular-nums hidden sm:block">{formatDateTime(item.updated_at, "fr")}</span>

        {canRetry && (
          <button
            type="button"
            onClick={handleRetry}
            disabled={busy(item.id)}
            className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors disabled:opacity-30 shrink-0"
            title={t("downloads.retry")}
          >
            {retrying ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="refresh" size="sm" />}
          </button>
        )}
        <button
          type="button"
          onClick={handleDelete}
          disabled={busy(item.id)}
          className="inline-flex items-center justify-center w-6 h-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
          title={isActive ? t("downloads.cancel") : t("downloads.delete")}
        >
          {deleting ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="trash" size="sm" />}
        </button>
      </div>
    </>
  );
}

export function AvailableDownloadsSection({
  latestFound,
  telegramAvailable,
}: {
  latestFound: LatestFoundPerLibraryDto[];
  telegramAvailable: TelegramAvailableGroupDto[];
}) {
  const { t } = useTranslation();
  const { busy, state, run } = useDownloadActions();
  const [sort, setSort] = useState<AvailableSortKey>("recent");
  const [filterLib, setFilterLib] = useState<string>("all");
  const [sourceFilter, setSourceFilter] = useState<AvailableSourceFilter>("all");
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const allResults = buildAvailableGroups(latestFound, telegramAvailable);
  const sorted = sortAvailableGroups(
    filterAvailableGroups(allResults, { libraryId: filterLib, source: sourceFilter }),
    sort,
  );
  const libraries = availableLibraries(allResults);

  /** The verb an item currently has in flight, so a row can pick its spinner. */
  function pendingVerb(id: string) {
    const current = state(id);
    return current.status === "pending" ? current.verb : null;
  }

  function handleDeleteRelease(entryId: string, releaseIdx: number, blacklist = false) {
    run(
      { target: "release", verb: blacklist ? "dismiss" : "delete" },
      `${entryId}-${releaseIdx}`,
      { context: { release: { entryId, releaseIndex: releaseIdx } } },
    );
  }

  function handleDeleteProwlarrEntries(group: AvailableGroup) {
    const ids = Array.from(new Set(
      group.sources
        .filter((source): source is Extract<AvailableSource, { kind: "prowlarr" } | { kind: "ddl" }> =>
          source.kind === "prowlarr" || source.kind === "ddl"
        )
        .map(source => source.entryId),
    ));
    if (ids.length === 0) return;
    run({ target: "entries", verb: "delete" }, group.key, { context: { entryIds: ids } });
  }

  function handleDdlDownload(group: AvailableGroup, source: Extract<AvailableSource, { kind: "ddl" }>) {
    const provider = source.release.provider;
    const externalId = source.release.external_id;
    if (!provider || !externalId) return;
    const volume =
      source.release.matched_missing_volumes[0] ?? source.release.all_volumes[0] ?? null;
    run({ target: "ddl", verb: "start" }, `${source.entryId}-${source.releaseIndex}`, {
      context: {
        ddlStart: {
          provider,
          body: {
            external_id: externalId,
            filename: source.release.title,
            size: source.release.size,
            library_id: group.library_id,
            series_name: group.series_name,
            volume_number: volume,
          },
        },
      },
    });
  }

  function handleTelegramDownload(bookId: string) {
    run({ target: "telegram", verb: "start" }, bookId);
  }

  function handleTelegramDismiss(bookId: string) {
    run({ target: "telegram", verb: "dismiss" }, bookId);
  }

  function handleTelegramDelete(bookId: string) {
    run({ target: "telegram", verb: "delete" }, bookId);
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

  // The blacklist panel is an admin list, not a download target: it keeps its
  // own read and its own local filtering. It still refetches when it opens and
  // whenever a refresh brings new detections, so a release that was just hidden
  // from the list above shows up here too.
  useEffect(() => {
    if (!showBlacklist) return;
    let cancelled = false;
    setLoadingBlacklist(true);
    browserApi.downloads.blacklist
      .list()
      .then((items) => {
        if (!cancelled) setBlacklistItems(items);
      })
      .catch(() => {
        /* keep the previous list when the read fails */
      })
      .finally(() => {
        if (!cancelled) setLoadingBlacklist(false);
      });
    return () => {
      cancelled = true;
    };
  }, [showBlacklist, latestFound]);

  async function handleUnblacklist(itemId: string) {
    try {
      await browserApi.downloads.blacklist.remove(itemId);
      setBlacklistItems((prev) => prev.filter((h) => h.id !== itemId));
    } catch {
      // keep the entry when the remove is rejected
    }
  }

  return (
    <div className="mt-10">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 mb-4">
        <h2 className="text-xl font-bold text-foreground flex items-center gap-2">
          <Icon name="search" size="lg" />
          {t("downloads.availableTitle")}
          <span className="text-sm font-normal text-muted-foreground">({sorted.length})</span>
          <button
            onClick={() => setShowBlacklist((open) => !open)}
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
            <option value="ddl">{t("downloads.sourceDdl")}</option>
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
          const visibleSources = visibleAvailableSources(r, sourceFilter);
          const {
            prowlarr: prowlarrCount,
            ddl: ddlCount,
            telegram: telegramCount,
            failed: failedReleaseCount,
            seeders: topSeeders,
          } = summarizeAvailableSources(visibleSources);

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
                    <span className="text-3xs text-muted-foreground hidden sm:inline shrink-0">{r.library_name}</span>
                  )}
                </div>

                <div className="flex w-full sm:w-auto items-center justify-end gap-2 shrink-0 pl-5 sm:pl-0">
                  {prowlarrCount > 0 && (
                    <span className="text-3xs text-muted-foreground shrink-0">
                      {prowlarrCount} Prowlarr
                    </span>
                  )}
                  {ddlCount > 0 && (
                    <span className="text-3xs text-success shrink-0">
                      {ddlCount} {t("downloads.sourceDdl")}
                    </span>
                  )}
                  {telegramCount > 0 && (
                    <span className="text-3xs text-info shrink-0">
                      {telegramCount} TG
                    </span>
                  )}
                  {(() => {
                    const newest = r.updated_at;
                    return newest ? (
                      <span className="text-3xs text-muted-foreground shrink-0 tabular-nums">{formatDateTime(newest, "fr")}</span>
                    ) : null;
                  })()}
                  {failedReleaseCount > 0 && (
                    <span className="text-3xs px-1.5 py-0.5 rounded-full font-medium bg-destructive/20 text-destructive" title={`${failedReleaseCount} failed`}>
                      {failedReleaseCount}!
                    </span>
                  )}
                  <span className="text-3xs px-1.5 py-0.5 rounded-full font-medium bg-warning/20 text-warning" title={`${r.missing_count} missing`}>
                    {r.missing_count} {t("downloads.missing")}
                  </span>
                  {topSeeders > 0 && (
                    <span className={`text-xs font-bold tabular-nums ${
                      topSeeders >= 10 ? "text-success" : topSeeders >= 3 ? "text-warning" : "text-error"
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
                    const aVol = Math.min(...availableSourceVolumes(a), 9999);
                    const bVol = Math.min(...availableSourceVolumes(b), 9999);
                    if (aVol !== bVol) return aVol - bVol;
                    return a.kind.localeCompare(b.kind);
                  }).map((source) => source.kind !== "telegram" ? (
                    <div
                      key={`${source.kind}-${source.entryId}-${source.releaseIndex}`}
                      className={`flex items-center gap-1.5 sm:gap-2 px-2 sm:px-3 py-1.5 pl-7 sm:pl-9 text-2xs hover:bg-muted/20 border-b border-border/10 last:border-b-0 transition-colors ${source.release.has_failed ? "bg-destructive/5 hover:bg-destructive/10" : ""}`}
                    >
                      {source.release.has_failed && (
                        <span className="px-1.5 py-px rounded bg-destructive/20 text-destructive font-medium shrink-0" title={t("downloads.failedBefore", { count: 1 })}>!</span>
                      )}
                      {source.release.match_confidence === "review" && (
                        <span className="px-1.5 py-px rounded bg-warning/20 text-warning font-medium shrink-0" title={source.release.match_reasons?.join(", ")}>
                          À vérifier
                        </span>
                      )}
                      <span className={`px-1.5 py-px rounded font-medium shrink-0 ${source.kind === "ddl" ? "bg-success/15 text-success" : "bg-primary/10 text-primary"}`}>
                        {source.kind === "ddl" ? (source.release.provider ?? "DDL") : "Prowlarr"}
                      </span>
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
                          source.release.seeders >= 10 ? "text-success" : source.release.seeders >= 3 ? "text-warning" : "text-error"
                        }`}>{source.release.seeders}S</span>
                      )}
                      <span className="text-muted-foreground shrink-0 tabular-nums">{(source.release.size / 1024 / 1024).toFixed(0)}MB</span>
                      <div className="flex items-center gap-0.5 ml-auto shrink-0">
                        {source.kind === "ddl" ? (
                          <button
                            type="button"
                            onClick={() => handleDdlDownload(r, source)}
                            disabled={busy(`${source.entryId}-${source.releaseIndex}`)}
                            title={t("ddlProviders.download")}
                            className="inline-flex items-center justify-center w-7 h-7 rounded-md text-success hover:bg-success/10 transition-colors disabled:opacity-30 shrink-0"
                          >
                            {pendingVerb(`${source.entryId}-${source.releaseIndex}`) === "start"
                              ? <Icon name="spinner" size="sm" className="animate-spin" />
                              : <Icon name="download" size="sm" />}
                          </button>
                        ) : (
                          source.release.download_url && (
                            <DownloadClientButton
                              downloadUrl={source.release.download_url}
                              releaseId={`${source.entryId}-${source.releaseIndex}`}
                              libraryId={r.library_id}
                              seriesName={r.series_name}
                              expectedVolumes={source.release.matched_missing_volumes}
                              allVolumes={source.release.all_volumes}
                              requiresReview={source.release.match_confidence === "review"}
                            />
                          )
                        )}
                        <button
                          type="button"
                          onClick={() => handleDeleteRelease(source.entryId, source.releaseIndex, true)}
                          disabled={busy(`${source.entryId}-${source.releaseIndex}`)}
                          title={t("downloads.blacklist")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-30 shrink-0"
                        >
                          {pendingVerb(`${source.entryId}-${source.releaseIndex}`) === "dismiss"
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : <Icon name="x" size="sm" />}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleDeleteRelease(source.entryId, source.releaseIndex)}
                          disabled={busy(`${source.entryId}-${source.releaseIndex}`)}
                          title={t("downloads.delete")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
                        >
                          {pendingVerb(`${source.entryId}-${source.releaseIndex}`) === "delete"
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : <Icon name="trash" size="sm" />}
                        </button>
                      </div>
                    </div>
                  ) : (
                    <div
                      key={`telegram-${source.book.id}`}
                      className={`flex items-center gap-1.5 sm:gap-2 px-2 sm:px-3 py-1.5 pl-7 sm:pl-9 text-2xs border-b border-border/10 last:border-b-0 transition-colors ${source.book.status === "failed" ? "bg-destructive/5 hover:bg-destructive/10" : "hover:bg-muted/20"}`}
                    >
                      <span className="px-1.5 py-px rounded bg-info/15 text-info font-medium shrink-0">TG</span>
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
                          disabled={busy(source.book.id)}
                          title={source.book.status === "failed" ? t("downloads.retry") : t("telegramMonitor.download")}
                          className={`inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-30 ${source.book.status === "failed" ? "text-destructive hover:text-foreground hover:bg-muted" : "text-primary hover:bg-primary/10"}`}
                        >
                          {pendingVerb(source.book.id) === "start" || pendingVerb(source.book.id) === "retry"
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : source.book.status === "failed" ? <Icon name="refresh" size="sm" /> : <Icon name="download" size="sm" />}
                        </button>
                        )}
                        <button
                          type="button"
                          onClick={() => handleTelegramDismiss(source.book.id)}
                          disabled={busy(source.book.id)}
                          title={t("downloads.blacklist")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-30 shrink-0"
                        >
                          {pendingVerb(source.book.id) === "dismiss"
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : <Icon name="x" size="sm" />}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleTelegramDelete(source.book.id)}
                          disabled={busy(source.book.id)}
                          title={t("downloads.delete")}
                          className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30 shrink-0"
                        >
                          {pendingVerb(source.book.id) === "delete"
                            ? <Icon name="spinner" size="sm" className="animate-spin" />
                            : <Icon name="trash" size="sm" />}
                        </button>
                      </div>
                    </div>
                  ))}
                  {prowlarrCount + ddlCount > 0 && (
                  <div className="flex justify-end px-2 sm:px-3 py-0.5 pl-7 sm:pl-9">
                    <button
                      type="button"
                      onClick={() => handleDeleteProwlarrEntries(r)}
                      disabled={busy(r.key)}
                      className="text-3xs text-muted-foreground hover:text-destructive transition-colors disabled:opacity-30"
                    >
                      {pendingVerb(r.key) === "delete"
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
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

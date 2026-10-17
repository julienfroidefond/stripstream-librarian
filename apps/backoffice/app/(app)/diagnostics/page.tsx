import type { ReactNode } from "react";
import { Badge, Card, CardContent, CardDescription, CardHeader, CardTitle, Icon, ProgressBar, SettingsPageHeader, StatBox } from "@/app/components/ui";
import { fetchDiagnostics, type DiagnosticsResponse, type DiagnosticsService } from "@/lib/api";
import { formatBytes, formatDateTime, formatNumber, formatUptime } from "@/lib/format";
import { getServerTranslations } from "@/lib/i18n/server";
import { CopyDiagnosticsButton, PurgeCacheButton, RefreshDiagnosticsButton } from "./DiagnosticsActions";
import { DiagnosticsProbePanel } from "./DiagnosticsProbe";

export const dynamic = "force-dynamic";

function InfoRow({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-border/50 py-2 last:border-0">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-right text-sm font-medium text-foreground">{value}</span>
    </div>
  );
}

function StatusBadge({ ok, okLabel, koLabel }: { ok: boolean; okLabel: string; koLabel: string }) {
  return <Badge variant={ok ? "success" : "error"}>{ok ? okLabel : koLabel}</Badge>;
}

/** Last run on top, next run underneath: two dates rarely fit on one line. */
function LastNext({
  last,
  next,
  locale,
  neverLabel,
}: {
  last: string | null;
  next: string | null;
  locale: string;
  neverLabel: string;
}) {
  return (
    <div className="space-y-0.5 whitespace-nowrap text-xs">
      <div className="text-muted-foreground">{last ? formatDateTime(last, locale) : neverLabel}</div>
      <div className="text-foreground">→ {next ? formatDateTime(next, locale) : neverLabel}</div>
    </div>
  );
}

export default async function DiagnosticsPage() {
  const { t, locale } = await getServerTranslations();

  let report: DiagnosticsResponse | null = null;
  let fetchError: string | null = null;
  try {
    report = await fetchDiagnostics();
  } catch (error) {
    fetchError = error instanceof Error ? error.message : String(error);
  }

  if (!report) {
    return (
      <div className="mx-auto max-w-7xl space-y-6">
        <SettingsPageHeader
          title={t("diagnostics.title")}
          description={t("diagnostics.subtitle")}
          icon="performance"
        />
        <Card hover={false} className="border-destructive/40">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-destructive">
              <Icon name="warning" size="md" />
              {t("diagnostics.unreachable")}
            </CardTitle>
            <CardDescription>{t("diagnostics.unreachableHint")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {fetchError && (
              <p className="break-all rounded-md bg-muted/50 p-3 font-mono text-xs text-muted-foreground">
                {fetchError}
              </p>
            )}
            <RefreshDiagnosticsButton label={t("diagnostics.retry")} />
          </CardContent>
        </Card>
      </div>
    );
  }

  const totalBooks = report.libraries.reduce((sum, library) => sum + library.book_count, 0);
  const totalSizeBytes = report.libraries.reduce((sum, library) => sum + library.size_bytes, 0);
  const queuedJobs = report.jobs.pending + report.jobs.running;
  const cacheHits = report.cache.hits + report.cache.misses;
  const integrityProblems =
    report.integrity.books_without_files +
    report.integrity.books_zero_pages +
    report.integrity.series_without_metadata +
    report.integrity.duplicate_volume_groups;

  const diskLabel = (id: string) => {
    switch (id) {
      case "libraries":
        return t("diagnostics.diskLibraries");
      case "thumbnails":
        return t("diagnostics.diskThumbnails");
      case "image_cache":
        return t("diagnostics.diskImageCache");
      default:
        return id;
    }
  };

  const serviceLabel = (service: DiagnosticsService) => {
    switch (service.service) {
      case "api":
        return t("diagnostics.serviceApi");
      case "indexer":
        return t("diagnostics.serviceIndexer");
      default:
        return service.service;
    }
  };

  return (
    <div className="mx-auto max-w-7xl space-y-6">
      <SettingsPageHeader
        title={t("diagnostics.title")}
        description={t("diagnostics.subtitle")}
        icon="performance"
        actions={
          <div className="flex flex-wrap gap-2">
            <CopyDiagnosticsButton />
            <RefreshDiagnosticsButton />
          </div>
        }
      />

      <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
        <StatBox value={report.version} label={t("diagnostics.apiVersion")} variant="primary" icon={<Icon name="layers" size="sm" />} />
        <StatBox value={formatUptime(report.uptime_seconds)} label={t("diagnostics.uptime")} variant="success" icon={<Icon name="clock" size="sm" />} />
        <StatBox value={formatNumber(totalBooks, locale)} label={t("diagnostics.totalBooks")} icon={<Icon name="books" size="sm" />} />
        <StatBox value={formatNumber(report.libraries.length, locale)} label={t("diagnostics.libraries")} icon={<Icon name="libraries" size="sm" />} />
        <StatBox value={formatBytes(totalSizeBytes, locale)} label={t("diagnostics.collectionSize")} icon={<Icon name="database" size="sm" />} />
        <StatBox value={formatNumber(queuedJobs, locale)} label={t("diagnostics.jobsInFlight")} variant={queuedJobs > 0 ? "primary" : "default"} icon={<Icon name="jobs" size="sm" />} />
        <StatBox
          value={formatNumber(report.jobs.stale_pending, locale)}
          label={t("diagnostics.stalePending")}
          variant={report.jobs.stale_pending > 0 ? "error" : "default"}
          icon={<Icon name="warning" size="sm" />}
        />
        <StatBox
          value={formatNumber(integrityProblems, locale)}
          label={t("diagnostics.integrity")}
          variant={integrityProblems > 0 ? "error" : "default"}
          icon={<Icon name="image" size="sm" />}
        />
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="performance" size="md" className="text-primary" />
              {t("diagnostics.services")}
            </CardTitle>
            <CardDescription>{t("diagnostics.servicesHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            {report.services.length === 0 ? (
              <p className="text-sm text-muted-foreground">{t("diagnostics.servicesUnknown")}</p>
            ) : (
              <ul className="space-y-3">
                {report.services.map((service) => (
                  <li
                    key={service.service}
                    className="flex flex-wrap items-center justify-between gap-2 border-b border-border/50 pb-3 last:border-0 last:pb-0"
                  >
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-sm font-medium text-foreground">{serviceLabel(service)}</span>
                      <StatusBadge
                        ok={service.online}
                        okLabel={t("diagnostics.online")}
                        koLabel={t("diagnostics.offline")}
                      />
                      {service.version !== report.version && (
                        <Badge variant="warning">{t("diagnostics.versionMismatch")}</Badge>
                      )}
                    </div>
                    <span className="font-mono text-xs text-muted-foreground">
                      v{service.version} · {t("diagnostics.uptime")} {formatUptime(service.uptime_seconds)} ·{" "}
                      {t("diagnostics.lastHeartbeat")} {formatUptime(service.seconds_since_seen)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>

        <Card hover={false}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="database" size="md" className="text-primary" />
              {t("diagnostics.database")}
            </CardTitle>
            <CardDescription>{t("diagnostics.databaseHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <InfoRow
              label={t("diagnostics.status")}
              value={
                <StatusBadge
                  ok={report.database.reachable}
                  okLabel={t("diagnostics.reachable")}
                  koLabel={t("diagnostics.unreachableStatus")}
                />
              }
            />
            <InfoRow label={t("diagnostics.latency")} value={`${report.database.latency_ms} ms`} />
            <InfoRow label={t("diagnostics.databaseSize")} value={formatBytes(report.database.size_bytes, locale)} />
            <InfoRow label={t("diagnostics.migrations")} value={formatNumber(report.database.migrations_applied, locale)} />
            <InfoRow label={t("diagnostics.latestMigration")} value={report.database.latest_migration ?? "—"} />
            <InfoRow
              label={t("diagnostics.connections")}
              value={
                report.database.max_connections > 0
                  ? `${report.database.active_connections} / ${report.database.max_connections}`
                  : "—"
              }
            />
          </CardContent>
        </Card>

        <Card hover={false}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="folder" size="md" className="text-highlight" />
              {t("diagnostics.disk")}
            </CardTitle>
            <CardDescription>{t("diagnostics.diskHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            {report.disks.map((disk) => (
              <div key={disk.id} className="space-y-1 border-b border-border/50 py-3 first:pt-0 last:border-0 last:pb-0">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="text-sm text-muted-foreground">{diskLabel(disk.id)}</span>
                  {disk.available ? (
                    <span className="text-sm font-medium text-foreground">
                      {formatBytes(disk.free_bytes, locale)} {t("diagnostics.diskFree")} /{" "}
                      {formatBytes(disk.total_bytes, locale)}
                    </span>
                  ) : (
                    <Badge variant="muted">{t("diagnostics.diskUnavailable")}</Badge>
                  )}
                </div>
                {disk.available && (
                  <ProgressBar
                    value={disk.used_percent}
                    max={100}
                    size="sm"
                    variant={disk.used_percent >= 90 ? "error" : disk.used_percent >= 80 ? "warning" : "success"}
                  />
                )}
                <p className="truncate font-mono text-xs text-muted-foreground">{disk.path}</p>
              </div>
            ))}
          </CardContent>
        </Card>

        <Card hover={false}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="performance" size="md" className="text-info" />
              {t("diagnostics.runtime")}
            </CardTitle>
            <CardDescription>{t("diagnostics.runtimeHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <InfoRow
              label={t("diagnostics.memoryRss")}
              value={report.runtime.rss_pretty ?? <span className="text-muted-foreground">{t("diagnostics.notAvailable")}</span>}
            />
            <InfoRow
              label={t("diagnostics.openFds")}
              value={
                report.runtime.open_fds === null
                  ? <span className="text-muted-foreground">{t("diagnostics.notAvailable")}</span>
                  : formatNumber(report.runtime.open_fds, locale)
              }
            />
            <InfoRow
              label={t("diagnostics.rendersInFlight")}
              value={`${formatNumber(report.runtime.renders_in_flight, locale)} / ${formatNumber(report.runtime.render_limit, locale)}`}
            />
          </CardContent>
        </Card>

        <Card hover={false}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="cache" size="md" className="text-info" />
              {t("diagnostics.cache")}
            </CardTitle>
            <CardDescription>{t("diagnostics.cacheHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <InfoRow label={t("diagnostics.memoryCache")} value={`${report.cache.memory_size_mb.toFixed(1)} / ${report.cache.memory_max_size_mb.toFixed(1)} MB`} />
            <InfoRow label={t("diagnostics.cachedPages")} value={formatNumber(report.cache.memory_page_count, locale)} />
            <InfoRow label={t("diagnostics.diskCache")} value={`${report.cache.disk_size_mb.toFixed(1)} MB`} />
            <InfoRow label={t("diagnostics.diskFiles")} value={formatNumber(report.cache.disk_file_count, locale)} />
            <InfoRow
              label={t("diagnostics.hitRate")}
              value={cacheHits > 0 ? `${Math.round((report.cache.hits / cacheHits) * 100)} %` : "—"}
            />
            <InfoRow label={t("diagnostics.cacheDirectory")} value={<span className="font-mono text-xs">{report.cache.directory}</span>} />
            <div className="pt-4">
              <PurgeCacheButton />
            </div>
          </CardContent>
        </Card>

        <Card hover={false}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="image" size="md" className="text-warning" />
              {t("diagnostics.thumbnails")}
            </CardTitle>
            <CardDescription>{t("diagnostics.thumbnailsHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <InfoRow label={t("diagnostics.storedFiles")} value={formatNumber(report.thumbnails.file_count, locale)} />
            <InfoRow label={t("diagnostics.totalSize")} value={formatBytes(report.thumbnails.total_size_bytes, locale)} />
            <InfoRow label={t("diagnostics.missingThumbnails")} value={formatNumber(report.thumbnails.missing, locale)} />
            <InfoRow label={t("diagnostics.pendingAnalysis")} value={formatNumber(report.thumbnails.pending_analysis, locale)} />
            <InfoRow label={t("diagnostics.thumbnailDirectory")} value={<span className="font-mono text-xs">{report.thumbnails.directory}</span>} />
          </CardContent>
        </Card>

        <Card hover={false} className={report.jobs.stale_pending > 0 ? "border-destructive/40" : undefined}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="jobs" size="md" className="text-highlight" />
              {t("diagnostics.jobs")}
            </CardTitle>
            <CardDescription>{t("diagnostics.jobsHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <InfoRow label={t("diagnostics.jobsTotal")} value={formatNumber(report.jobs.total, locale)} />
            <InfoRow label={t("diagnostics.jobsPending")} value={formatNumber(report.jobs.pending, locale)} />
            <InfoRow label={t("diagnostics.jobsRunning")} value={formatNumber(report.jobs.running, locale)} />
            <InfoRow label={t("diagnostics.jobsFailed24h")} value={formatNumber(report.jobs.failed_last_24h, locale)} />
            <InfoRow
              label={t("diagnostics.stalePending")}
              value={
                report.jobs.stale_pending > 0 ? (
                  <Badge variant="error">{formatNumber(report.jobs.stale_pending, locale)}</Badge>
                ) : (
                  formatNumber(0, locale)
                )
              }
            />
            {report.jobs.stale_pending > 0 && (
              <p className="pt-2 text-xs text-destructive">{t("diagnostics.stalePendingHint")}</p>
            )}
            {report.jobs.oldest_pending_seconds !== null && (
              <InfoRow
                label={t("diagnostics.oldestPending")}
                value={formatUptime(report.jobs.oldest_pending_seconds)}
              />
            )}

            <h3 className="mt-4 mb-2 text-sm font-semibold text-foreground">{t("diagnostics.jobsByType")}</h3>
            {report.jobs.by_type.length === 0 ? (
              <p className="text-sm text-muted-foreground">{t("diagnostics.noFailures")}</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                      <th className="py-2 pr-4 font-medium">{t("diagnostics.jobType")}</th>
                      <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.jobsPending")}</th>
                      <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.jobsRunning")}</th>
                      <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.jobsFailed24h")}</th>
                      <th className="py-2 font-medium">{t("diagnostics.lastFinished")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.jobs.by_type.map((job) => (
                      <tr key={job.job_type} className="border-b border-border/50 last:border-0">
                        <td className="py-2 pr-4">
                          <Badge variant={job.failed_last_24h > 0 ? "error" : "default"}>{job.job_type}</Badge>
                        </td>
                        <td className="py-2 pr-4 text-right">{formatNumber(job.pending, locale)}</td>
                        <td className="py-2 pr-4 text-right">{formatNumber(job.running, locale)}</td>
                        <td className="py-2 pr-4 text-right">{formatNumber(job.failed_last_24h, locale)}</td>
                        <td className="py-2 text-xs text-muted-foreground">
                          {job.last_finished_at ? formatDateTime(job.last_finished_at, locale) : t("diagnostics.never")}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <h3 className="mt-4 mb-2 text-sm font-semibold text-foreground">{t("diagnostics.recentFailures")}</h3>
            {report.jobs.recent_failures.length === 0 ? (
              <p className="text-sm text-muted-foreground">{t("diagnostics.noFailures")}</p>
            ) : (
              <ul className="space-y-2">
                {report.jobs.recent_failures.map((failure) => (
                  <li key={failure.id} className="rounded-md bg-destructive/5 p-3">
                    <div className="flex flex-wrap items-center gap-2 text-sm">
                      <Badge variant="error">{failure.job_type}</Badge>
                      {failure.library_name && <span className="font-medium text-foreground">{failure.library_name}</span>}
                      <span className="text-xs text-muted-foreground">
                        {failure.finished_at ? formatDateTime(failure.finished_at, locale) : "—"}
                      </span>
                    </div>
                    {failure.error && (
                      <p className="mt-1 break-words text-xs text-destructive">{failure.error}</p>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>

        <Card hover={false} className={integrityProblems > 0 ? "border-warning/40" : undefined}>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="database" size="md" className="text-warning" />
              {t("diagnostics.integrity")}
            </CardTitle>
            <CardDescription>{t("diagnostics.integrityHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <InfoRow label={t("diagnostics.booksWithoutFiles")} value={formatNumber(report.integrity.books_without_files, locale)} />
            <InfoRow label={t("diagnostics.booksZeroPages")} value={formatNumber(report.integrity.books_zero_pages, locale)} />
            <InfoRow label={t("diagnostics.seriesWithoutMetadata")} value={formatNumber(report.integrity.series_without_metadata, locale)} />
            <InfoRow label={t("diagnostics.duplicateVolumes")} value={formatNumber(report.integrity.duplicate_volume_groups, locale)} />
            {integrityProblems === 0 && (
              <p className="pt-2 text-sm text-success">{t("diagnostics.integrityClean")}</p>
            )}
          </CardContent>
        </Card>

        <Card hover={false} className="xl:col-span-2">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Icon name="link" size="md" className="text-info" />
              {t("diagnostics.probe")}
            </CardTitle>
            <CardDescription>{t("diagnostics.probeHint")}</CardDescription>
          </CardHeader>
          <CardContent>
            <DiagnosticsProbePanel />
          </CardContent>
        </Card>
      </div>

      <Card hover={false}>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="libraries" size="md" className="text-primary" />
            {t("diagnostics.libraries")}
          </CardTitle>
          <CardDescription>{t("diagnostics.librariesHint")}</CardDescription>
        </CardHeader>
        <CardContent>
          {report.libraries.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("diagnostics.noLibraries")}</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
                    <th className="py-2 pr-4 font-medium">{t("diagnostics.libraryName")}</th>
                    <th className="py-2 pr-4 font-medium">{t("diagnostics.status")}</th>
                    <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.books")}</th>
                    <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.pendingAnalysis")}</th>
                    <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.missingThumbnails")}</th>
                    <th className="py-2 pr-4 text-right font-medium">{t("diagnostics.totalSize")}</th>
                    <th className="py-2 pr-4 font-medium">{t("diagnostics.scan")}</th>
                    <th className="py-2 pr-4 font-medium">{t("diagnostics.metadata")}</th>
                    <th className="py-2 font-medium">{t("diagnostics.downloadDetection")}</th>
                  </tr>
                </thead>
                <tbody>
                  {report.libraries.map((library) => (
                    <tr key={library.id} className="border-b border-border/50 last:border-0">
                      <td className="py-2 pr-4 font-medium text-foreground">{library.name}</td>
                      <td className="py-2 pr-4">
                        <StatusBadge
                          ok={library.enabled}
                          okLabel={t("diagnostics.enabled")}
                          koLabel={t("diagnostics.disabled")}
                        />
                      </td>
                      <td className="py-2 pr-4 text-right">{formatNumber(library.book_count, locale)}</td>
                      <td className="py-2 pr-4 text-right">{formatNumber(library.pending_analysis, locale)}</td>
                      <td className="py-2 pr-4 text-right">{formatNumber(library.missing_thumbnails, locale)}</td>
                      <td className="py-2 pr-4 text-right">{formatBytes(library.size_bytes, locale)}</td>
                      <td className="py-2 pr-4">
                        <LastNext
                          last={library.last_scan_at}
                          next={library.next_scan_at}
                          locale={locale}
                          neverLabel={t("diagnostics.never")}
                        />
                      </td>
                      <td className="py-2 pr-4">
                        <LastNext
                          last={library.last_metadata_refresh_at}
                          next={library.next_metadata_refresh_at}
                          locale={locale}
                          neverLabel={t("diagnostics.never")}
                        />
                      </td>
                      <td className="py-2">
                        <LastNext
                          last={library.last_download_detection_at}
                          next={library.next_download_detection_at}
                          locale={locale}
                          neverLabel={t("diagnostics.never")}
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

import type { MetadataBatchReportDto, MetadataRefreshReportDto, ReadingStatusMatchReportDto, ReadingStatusPushReportDto, DownloadDetectionReportDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface JobSummaryBannerProps {
  job: {
    type: string;
    status: string;
    started_at: string | null;
    finished_at: string | null;
    error_opt: string | null;
    stats_json: {
      scanned_files: number;
      indexed_files: number;
      removed_files: number;
      errors: number;
      warnings: number;
    } | null;
    total_files: number | null;
    processed_files: number | null;
  };
  batchReport: MetadataBatchReportDto | null;
  refreshReport: MetadataRefreshReportDto | null;
  readingStatusReport: ReadingStatusMatchReportDto | null;
  readingStatusPushReport: ReadingStatusPushReportDto | null;
  downloadDetectionReport: DownloadDetectionReportDto | null;
  t: TranslateFunction;
  formatDuration: (start: string, end: string | null) => string;
}

export function JobSummaryBanner({
  job, batchReport, refreshReport, readingStatusReport, readingStatusPushReport, downloadDetectionReport, t, formatDuration,
}: JobSummaryBannerProps) {
  const isCompleted = job.status === "success";
  const isFailed = job.status === "failed";
  const isCancelled = job.status === "cancelled";
  const isMetadataBatch = job.type === "metadata_batch" || job.type === "metadata_batch_rematch";
  const isMetadataRefresh = job.type === "metadata_refresh" || job.type === "metadata_refresh_all";
  const isReadingStatusMatch = job.type === "reading_status_match";
  const isReadingStatusPush = job.type === "reading_status_push";
  const isDownloadDetection = job.type === "download_detection";
  const isThumbnailOnly = job.type === "thumbnail_rebuild" || job.type === "thumbnail_regenerate";

  if (isCompleted && job.started_at) {
    return (
      <div className="mb-6 p-4 rounded-xl bg-success/10 border border-success/30 flex items-start gap-3">
        <svg className="w-5 h-5 text-success mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <div className="text-sm text-success">
          <span className="font-semibold">{t("jobDetail.completedIn", { duration: formatDuration(job.started_at, job.finished_at) })}</span>
          {isMetadataBatch && batchReport && (
            <span className="ml-2 text-success/80">
              — {batchReport.auto_matched} {t("jobDetail.autoMatched").toLowerCase()}, {batchReport.already_linked} {t("jobDetail.alreadyLinked").toLowerCase()}, {batchReport.no_results} {t("jobDetail.noResults").toLowerCase()}, {batchReport.errors} {t("jobDetail.errors").toLowerCase()}
            </span>
          )}
          {isMetadataRefresh && refreshReport && (
            <span className="ml-2 text-success/80">
              — {refreshReport.refreshed} {t("jobDetail.refreshed").toLowerCase()}, {refreshReport.unchanged} {t("jobDetail.unchanged").toLowerCase()}, {refreshReport.errors} {t("jobDetail.errors").toLowerCase()}
            </span>
          )}
          {isReadingStatusMatch && readingStatusReport && (
            <span className="ml-2 text-success/80">
              — {readingStatusReport.linked} {t("jobDetail.linked").toLowerCase()}, {readingStatusReport.no_results} {t("jobDetail.noResults").toLowerCase()}, {readingStatusReport.ambiguous} {t("jobDetail.ambiguous").toLowerCase()}, {readingStatusReport.errors} {t("jobDetail.errors").toLowerCase()}
            </span>
          )}
          {isReadingStatusPush && readingStatusPushReport && (
            <span className="ml-2 text-success/80">
              — {readingStatusPushReport.pushed} {t("jobDetail.pushed").toLowerCase()}, {readingStatusPushReport.no_books} {t("jobDetail.noBooks").toLowerCase()}, {readingStatusPushReport.errors} {t("jobDetail.errors").toLowerCase()}
            </span>
          )}
          {isDownloadDetection && downloadDetectionReport && (
            <span className="ml-2 text-success/80">
              — {downloadDetectionReport.found} {t("jobDetail.downloadFound").toLowerCase()}, {downloadDetectionReport.not_found} {t("jobDetail.downloadNotFound").toLowerCase()}, {downloadDetectionReport.errors} {t("jobDetail.errors").toLowerCase()}
            </span>
          )}
          {!isMetadataBatch && !isMetadataRefresh && !isReadingStatusMatch && !isReadingStatusPush && !isDownloadDetection && job.stats_json && (
            <span className="ml-2 text-success/80">
              — {job.stats_json.scanned_files} {t("jobDetail.scanned").toLowerCase()}, {job.stats_json.indexed_files} {t("jobDetail.indexed").toLowerCase()}
              {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} ${t("jobDetail.removed").toLowerCase()}`}
              {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} ${t("jobDetail.warnings").toLowerCase()}`}
              {job.stats_json.errors > 0 && `, ${job.stats_json.errors} ${t("jobDetail.errors").toLowerCase()}`}
              {job.total_files != null && job.total_files > 0 && `, ${job.total_files} ${t("jobType.thumbnail_rebuild").toLowerCase()}`}
            </span>
          )}
          {!isMetadataBatch && !isMetadataRefresh && !isReadingStatusMatch && !isReadingStatusPush && !job.stats_json && isThumbnailOnly && job.total_files != null && (
            <span className="ml-2 text-success/80">
              — {job.processed_files ?? job.total_files} {t("jobDetail.generated").toLowerCase()}
            </span>
          )}
        </div>
      </div>
    );
  }

  if (isFailed) {
    return (
      <div className="mb-6 p-4 rounded-xl bg-destructive/10 border border-destructive/30 flex items-start gap-3">
        <svg className="w-5 h-5 text-destructive mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <div className="text-sm text-destructive">
          <span className="font-semibold">{t("jobDetail.jobFailed")}</span>
          {job.started_at && (
            <span className="ml-2 text-destructive/80">{t("jobDetail.failedAfter", { duration: formatDuration(job.started_at, job.finished_at) })}</span>
          )}
          {job.error_opt && (
            <p className="mt-1 text-destructive/70 font-mono text-xs break-all">{job.error_opt}</p>
          )}
        </div>
      </div>
    );
  }

  if (isCancelled) {
    return (
      <div className="mb-6 p-4 rounded-xl bg-muted border border-border flex items-start gap-3">
        <svg className="w-5 h-5 text-muted-foreground mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
        </svg>
        <span className="text-sm text-muted-foreground">
          <span className="font-semibold">{t("jobDetail.cancelled")}</span>
          {job.started_at && (
            <span className="ml-2">{t("jobDetail.failedAfter", { duration: formatDuration(job.started_at, job.finished_at) })}</span>
          )}
        </span>
      </div>
    );
  }

  return null;
}

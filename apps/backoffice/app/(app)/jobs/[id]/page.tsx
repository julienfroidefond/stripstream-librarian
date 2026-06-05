export const dynamic = "force-dynamic";

import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch, getMetadataBatchReport, getMetadataBatchResults, getMetadataRefreshReport, getReadingStatusMatchReport, getReadingStatusMatchResults, getReadingStatusPushReport, getReadingStatusPushResults, getDownloadDetectionReport, getDownloadDetectionResults, MetadataBatchReportDto, MetadataBatchResultDto, MetadataRefreshReportDto, ReadingStatusMatchReportDto, ReadingStatusMatchResultDto, ReadingStatusPushReportDto, ReadingStatusPushResultDto, DownloadDetectionReportDto, DownloadDetectionResultDto } from "@/lib/api";
import { JobDetailLive } from "@/app/components/JobDetailLive";
import { getServerTranslations } from "@/lib/i18n/server";
import { JobSummaryBanner } from "./components/JobSummaryBanner";
import { JobOverviewCard } from "./components/JobOverviewCard";
import { JobTimelineCard } from "./components/JobTimelineCard";
import { JobProgressCard, IndexStatsCard, ThumbnailStatsCard } from "./components/JobProgressCard";
import { MetadataBatchReportCard, MetadataRefreshReportCard, MetadataRefreshChangesCard } from "./components/MetadataReportCards";
import { MetadataBatchQuickMatch } from "./components/MetadataBatchQuickMatch";
import { ReadingStatusMatchReportCard, ReadingStatusMatchResultsCard, ReadingStatusPushReportCard, ReadingStatusPushResultsCard } from "./components/ReadingStatusReportCards";
import { DownloadDetectionReportCard, DownloadDetectionErrorsCard, RssSnapshotCard } from "./components/DownloadDetectionCards";
import { DownloadDetectionAvailableResults } from "./components/DownloadDetectionAvailableResults";
import { TelegramSyncResultsCard } from "./components/TelegramSyncResultsCard";
import { TelegramIncrementalResultsCard } from "./components/TelegramIncrementalResultsCard";
import { JobErrorsCard } from "./components/JobErrorsCard";
import { JobEventsCard, type JobEvent } from "./components/JobEventsCard";

interface JobDetailPageProps {
  params: Promise<{ id: string }>;
}

interface JobDetails {
  id: string;
  library_id: string | null;
  library_name: string | null;
  book_id: string | null;
  type: string;
  status: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  phase2_started_at: string | null;
  generating_thumbnails_started_at: string | null;
  current_file: string | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  stats_json: {
    scanned_files?: number;
    indexed_files?: number;
    removed_files?: number;
    errors?: number;
    warnings?: number;
    rss_releases?: Array<{ title: string; indexer?: string | null; size: number; seeders?: number | null; leechers?: number | null; publish_date?: string | null; categories?: string[] | null }>;
    synced?: number;
    new_books?: number;
    series_searched?: number;
    all_series?: Array<{ series_name: string; book_count: number; extracted_names?: string[] }>;
    matched_series?: Array<{ telegram_name: string; series_id: string; series_name: string; book_count: number }>;
    sources_scanned?: number;
    sources?: Array<{ username: string; new_books: number }>;
    recent_books?: Array<{ filename: string; series_name: string | null; volume_number: number | null; channel: string }>;
  } | null;
  error_opt: string | null;
}

interface JobError {
  id: string;
  file_path: string;
  error_message: string;
  created_at: string;
}

async function getJobDetails(jobId: string): Promise<JobDetails | null> {
  try {
    return await apiFetch<JobDetails>(`/index/jobs/${jobId}`);
  } catch {
    return null;
  }
}

async function getJobErrors(jobId: string): Promise<JobError[]> {
  try {
    return await apiFetch<JobError[]>(`/index/jobs/${jobId}/errors`);
  } catch {
    return [];
  }
}

function formatDuration(start: string, end: string | null): string {
  const startDate = new Date(start);
  const endDate = end ? new Date(end) : new Date();
  const diff = endDate.getTime() - startDate.getTime();

  if (diff < 60000) return `${Math.floor(diff / 1000)}s`;
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ${Math.floor((diff % 60000) / 1000)}s`;
  return `${Math.floor(diff / 3600000)}h ${Math.floor((diff % 3600000) / 60000)}m`;
}

function formatSpeed(count: number, durationMs: number): string {
  if (durationMs === 0 || count === 0) return "-";
  return `${(count / (durationMs / 1000)).toFixed(1)}/s`;
}

export default async function JobDetailPage({ params }: JobDetailPageProps) {
  const { id } = await params;
  const [job, errors, events] = await Promise.all([
    getJobDetails(id),
    getJobErrors(id),
    apiFetch<JobEvent[]>(`/index/jobs/${id}/events`).catch(() => []),
  ]);

  if (!job) {
    notFound();
  }

  const { t, locale } = await getServerTranslations();

  const JOB_TYPE_INFO: Record<string, { label: string; description: string; isThumbnailOnly: boolean }> = {
    rebuild: { label: t("jobType.rebuildLabel"), description: t("jobType.rebuildDesc"), isThumbnailOnly: false },
    full_rebuild: { label: t("jobType.full_rebuildLabel"), description: t("jobType.full_rebuildDesc"), isThumbnailOnly: false },
    rescan: { label: t("jobType.rescanLabel"), description: t("jobType.rescanDesc"), isThumbnailOnly: false },
    thumbnail_rebuild: { label: t("jobType.thumbnail_rebuildLabel"), description: t("jobType.thumbnail_rebuildDesc"), isThumbnailOnly: true },
    thumbnail_regenerate: { label: t("jobType.thumbnail_regenerateLabel"), description: t("jobType.thumbnail_regenerateDesc"), isThumbnailOnly: true },
    cbr_to_cbz: { label: t("jobType.cbr_to_cbzLabel"), description: t("jobType.cbr_to_cbzDesc"), isThumbnailOnly: false },
    metadata_batch: { label: t("jobType.metadata_batchLabel"), description: t("jobType.metadata_batchDesc"), isThumbnailOnly: false },
    metadata_batch_rematch: { label: t("jobType.metadata_batch_rematchLabel"), description: t("jobType.metadata_batch_rematchDesc"), isThumbnailOnly: false },
    metadata_refresh: { label: t("jobType.metadata_refreshLabel"), description: t("jobType.metadata_refreshDesc"), isThumbnailOnly: false },
    metadata_refresh_all: { label: t("jobType.metadata_refresh_allLabel"), description: t("jobType.metadata_refresh_allDesc"), isThumbnailOnly: false },
    reading_status_match: { label: t("jobType.reading_status_matchLabel"), description: t("jobType.reading_status_matchDesc"), isThumbnailOnly: false },
    reading_status_push: { label: t("jobType.reading_status_pushLabel"), description: t("jobType.reading_status_pushDesc"), isThumbnailOnly: false },
    download_detection: { label: t("jobType.download_detectionLabel"), description: t("jobType.download_detectionDesc"), isThumbnailOnly: false },
    prowlarr_rss: { label: t("jobType.prowlarr_rssLabel"), description: t("jobType.prowlarr_rssDesc"), isThumbnailOnly: false },
    telegram_sync: { label: t("jobType.telegram_syncLabel"), description: t("jobType.telegram_syncDesc"), isThumbnailOnly: false },
    telegram_sync_incremental: { label: t("jobType.telegram_sync_incrementalLabel"), description: t("jobType.telegram_sync_incrementalDesc"), isThumbnailOnly: false },
  };

  const isMetadataBatch = job.type === "metadata_batch" || job.type === "metadata_batch_rematch";
  const isMetadataRefresh = job.type === "metadata_refresh" || job.type === "metadata_refresh_all";
  const isReadingStatusMatch = job.type === "reading_status_match";
  const isReadingStatusPush = job.type === "reading_status_push";
  const isDownloadDetection = job.type === "download_detection";
  const isRssPoll = job.type === "prowlarr_rss";
  const isTelegramFullSync = job.type === "telegram_sync";
  const isTelegramIncremental = job.type === "telegram_sync_incremental";
  const isTelegramSync = isTelegramFullSync || isTelegramIncremental;

  let batchReport: MetadataBatchReportDto | null = null;
  let batchResults: MetadataBatchResultDto[] = [];
  if (isMetadataBatch) {
    [batchReport, batchResults] = await Promise.all([
      getMetadataBatchReport(id).catch(() => null),
      getMetadataBatchResults(id).catch(() => []),
    ]);
  }

  let refreshReport: MetadataRefreshReportDto | null = null;
  if (isMetadataRefresh) {
    refreshReport = await getMetadataRefreshReport(id).catch(() => null);
  }

  let readingStatusReport: ReadingStatusMatchReportDto | null = null;
  let readingStatusResults: ReadingStatusMatchResultDto[] = [];
  if (isReadingStatusMatch) {
    [readingStatusReport, readingStatusResults] = await Promise.all([
      getReadingStatusMatchReport(id).catch(() => null),
      getReadingStatusMatchResults(id).catch(() => []),
    ]);
  }

  let readingStatusPushReport: ReadingStatusPushReportDto | null = null;
  let readingStatusPushResults: ReadingStatusPushResultDto[] = [];
  if (isReadingStatusPush) {
    [readingStatusPushReport, readingStatusPushResults] = await Promise.all([
      getReadingStatusPushReport(id).catch(() => null),
      getReadingStatusPushResults(id).catch(() => []),
    ]);
  }

  let downloadDetectionReport: DownloadDetectionReportDto | null = null;
  let downloadDetectionResults: DownloadDetectionResultDto[] = [];
  let downloadDetectionErrors: DownloadDetectionResultDto[] = [];
  let qbConfigured = false;
  if (isDownloadDetection || isRssPoll) {
    [downloadDetectionReport, downloadDetectionResults, downloadDetectionErrors, qbConfigured] = await Promise.all([
      getDownloadDetectionReport(id).catch(() => null),
      getDownloadDetectionResults(id, "found").catch(() => []),
      getDownloadDetectionResults(id, "error").catch(() => []),
      apiFetch<{ url?: string; username?: string }>("/settings/qbittorrent")
        .then(d => !!(d?.url?.trim() && d?.username?.trim()))
        .catch(() => false),
    ]);
  }

  const typeInfo = JOB_TYPE_INFO[job.type] ?? { label: job.type, description: null, isThumbnailOnly: false };
  const { isThumbnailOnly } = typeInfo;

  const durationMs = job.started_at
    ? new Date(job.finished_at || new Date()).getTime() - new Date(job.started_at).getTime()
    : 0;

  const isCompleted = job.status === "success";
  const isFailed = job.status === "failed";
  const isCancelled = job.status === "cancelled";
  const isTerminal = isCompleted || isFailed || isCancelled;
  const isExtractingPages = job.status === "extracting_pages";
  const isThumbnailPhase = job.status === "generating_thumbnails";
  const isPhase2 = isExtractingPages || isThumbnailPhase;

  const progressTitle = isMetadataBatch
    ? t("jobDetail.metadataSearch")
    : isMetadataRefresh
    ? t("jobDetail.metadataRefresh")
    : isReadingStatusMatch
    ? t("jobDetail.readingStatusMatch")
    : isReadingStatusPush
    ? t("jobDetail.readingStatusPush")
    : isDownloadDetection
    ? t("jobDetail.downloadDetection")
    : isRssPoll
    ? t("jobType.prowlarr_rss")
    : isTelegramFullSync
    ? t("jobType.telegram_sync")
    : isTelegramIncremental
    ? t("jobType.telegram_sync_incremental")
    : isThumbnailOnly
      ? t("jobType.thumbnail_rebuild")
      : isExtractingPages
        ? t("jobDetail.phase2a")
        : isThumbnailPhase
          ? t("jobDetail.phase2b")
          : t("jobDetail.phase1");

  const progressDescription = isMetadataBatch
    ? t("jobDetail.metadataSearchDesc")
    : isMetadataRefresh
    ? t("jobDetail.metadataRefreshDesc")
    : isReadingStatusMatch
    ? t("jobDetail.readingStatusMatchDesc")
    : isReadingStatusPush
    ? t("jobDetail.readingStatusPushDesc")
    : isDownloadDetection
    ? t("jobDetail.downloadDetectionDesc")
    : isRssPoll
    ? t("jobType.prowlarr_rssDesc")
    : isThumbnailOnly
      ? undefined
      : isExtractingPages
        ? t("jobDetail.phase2aDesc")
        : isThumbnailPhase
          ? t("jobDetail.phase2bDesc")
          : t("jobDetail.phase1Desc");

  return (
    <>
      <JobDetailLive jobId={id} isTerminal={isTerminal} />
      <div className="mb-6">
        <Link
          href="/jobs"
          className="inline-flex items-center text-sm text-muted-foreground hover:text-primary transition-colors duration-200"
        >
          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
          {t("jobDetail.backToJobs")}
        </Link>
        <h1 className="text-3xl font-bold text-foreground mt-2">{t("jobDetail.title")}</h1>
      </div>

      <JobSummaryBanner
        job={job}
        batchReport={batchReport}
        refreshReport={refreshReport}
        readingStatusReport={readingStatusReport}
        readingStatusPushReport={readingStatusPushReport}
        downloadDetectionReport={downloadDetectionReport}
        t={t}
        formatDuration={formatDuration}
      />

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <JobOverviewCard job={job} typeInfo={typeInfo} t={t} formatDuration={formatDuration} />
        <JobTimelineCard job={job} isThumbnailOnly={isThumbnailOnly} t={t} locale={locale} formatDuration={formatDuration} />

        {!isTelegramFullSync && (
          <JobProgressCard
            job={job}
            isThumbnailOnly={isThumbnailOnly}
            progressTitle={progressTitle}
            progressDescription={progressDescription}
            t={t}
            formatDuration={formatDuration}
            formatSpeed={formatSpeed}
          />
        )}

        {/* Index Statistics */}
        {job.stats_json && !isThumbnailOnly && !isMetadataBatch && !isMetadataRefresh && !isReadingStatusMatch && !isReadingStatusPush && !isDownloadDetection && !isRssPoll && !isTelegramSync && (
          <IndexStatsCard job={job} t={t} formatDuration={formatDuration} formatSpeed={formatSpeed} durationMs={durationMs} />
        )}

        {/* Thumbnail statistics */}
        {isThumbnailOnly && isCompleted && job.total_files != null && (
          <ThumbnailStatsCard job={job} t={t} formatDuration={formatDuration} formatSpeed={formatSpeed} durationMs={durationMs} />
        )}

        {/* Metadata batch */}
        {isMetadataBatch && batchReport && <MetadataBatchReportCard report={batchReport} t={t} />}
        {isMetadataRefresh && refreshReport && <MetadataRefreshReportCard report={refreshReport} t={t} />}
        {isMetadataRefresh && refreshReport && <MetadataRefreshChangesCard report={refreshReport} libraryId={job.library_id} t={t} />}

        {/* Reading status */}
        {isReadingStatusMatch && readingStatusReport && <ReadingStatusMatchReportCard report={readingStatusReport} t={t} />}
        {isReadingStatusMatch && <ReadingStatusMatchResultsCard results={readingStatusResults} libraryId={job.library_id} t={t} />}
        {isReadingStatusPush && readingStatusPushReport && <ReadingStatusPushReportCard report={readingStatusPushReport} t={t} />}
        {isReadingStatusPush && <ReadingStatusPushResultsCard results={readingStatusPushResults} libraryId={job.library_id} t={t} />}

        {/* Download detection + Prowlarr RSS (partagent les mêmes events/endpoints) */}
        {(isDownloadDetection || isRssPoll) && downloadDetectionReport && <DownloadDetectionReportCard report={downloadDetectionReport} t={t} />}
        {(isDownloadDetection || isRssPoll) && <DownloadDetectionErrorsCard results={downloadDetectionErrors} t={t} />}
        {(isDownloadDetection || isRssPoll) && downloadDetectionResults.length > 0 && (
          <div className="lg:col-span-2">
            <DownloadDetectionAvailableResults
              results={downloadDetectionResults}
              libraryId={job.library_id || ""}
              libraryName={job.library_name || ""}
            />
          </div>
        )}

        {/* Metadata batch results */}
        {isMetadataBatch && <MetadataBatchQuickMatch results={batchResults} libraryId={job.library_id} />}

        {/* Telegram full sync results */}
        {isTelegramFullSync && job.stats_json && (
          <TelegramSyncResultsCard
            new_books={job.stats_json.new_books ?? 0}
            series_searched={job.stats_json.series_searched ?? 0}
            all_series={job.stats_json.all_series ?? []}
            matched_series={job.stats_json.matched_series ?? []}
            t={t}
          />
        )}

        {/* Telegram incremental sync results */}
        {isTelegramIncremental && job.stats_json && (
          <TelegramIncrementalResultsCard
            new_books={job.stats_json.new_books ?? 0}
            sources_scanned={job.stats_json.sources_scanned ?? 0}
            sources={job.stats_json.sources ?? []}
            recent_books={job.stats_json.recent_books ?? []}
            t={t}
          />
        )}

        {/* Job events */}
        <JobEventsCard events={events} t={t} locale={locale} />

        {/* File errors */}
        <JobErrorsCard errors={errors} t={t} locale={locale} />

        {/* RSS snapshot — flux brut Prowlarr, disponible pour les 5 derniers jobs seulement */}
        {isRssPoll && job.stats_json?.rss_releases && job.stats_json.rss_releases.length > 0 && (
          <RssSnapshotCard releases={job.stats_json.rss_releases} t={t} />
        )}
      </div>
    </>
  );
}

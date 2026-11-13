export const dynamic = "force-dynamic";

import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch, JobTypeCatalogDto, listJobTypes } from "@/lib/api";
import { JobDetailLive } from "@/app/components/JobDetailLive";
import { getServerTranslations } from "@/lib/i18n/server";
import {
  jobSummaryReports,
  resolveJobProgressCopy,
  resolveJobTypeDescriptor,
} from "@/lib/jobs/descriptors";
import { jobReportLoaders } from "@/lib/jobs/serverReports";
import { JobSummaryBanner } from "./components/JobSummaryBanner";
import { JobOverviewCard } from "./components/JobOverviewCard";
import { JobTimelineCard } from "./components/JobTimelineCard";
import { JobProgressCard, IndexStatsCard, ThumbnailStatsCard } from "./components/JobProgressCard";
import { RssSnapshotCard } from "./components/DownloadDetectionCards";
import { JobResultCards } from "./components/JobResultCards";
import { JobErrorsCard } from "./components/JobErrorsCard";
import { JobEventsCard, type JobEvent } from "./components/JobEventsCard";
import { formatDuration, formatRate } from "@/lib/format";
import { isCompletedJobStatus, isPartialJobStatus, isTerminalJobStatus } from "@/lib/jobStatus";

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
    /// DDL search: provider calls attempted vs failed, for the partial banner.
    attempted?: number;
    failed_providers?: string[];
    rss_indexer_stats?: Array<{ id?: number; name?: string; count?: number; deduped?: number; error?: string }>;
    rss_releases?: Array<{ title: string; indexer?: string | null; size: number; seeders?: number | null; leechers?: number | null; publish_date?: string | null; categories?: string[] | null }>;
    synced?: number;
    new_books?: number;
    series_searched?: number;
    all_series?: Array<{ series_name: string; book_count: number; extracted_names?: string[] }>;
    matched_series?: Array<{ telegram_name: string; series_id: string; series_name: string; book_count: number }>;
    sources_scanned?: number;
    sources_failed?: number;
    persistence_errors?: number;
    sources?: Array<{ username: string; new_books: number; messages_scanned?: number; error?: string; persistence_errors?: number }>;
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

export default async function JobDetailPage({ params }: JobDetailPageProps) {
  const { id } = await params;
  const [job, errors, events, jobTypeCatalog] = await Promise.all([
    getJobDetails(id),
    getJobErrors(id),
    apiFetch<JobEvent[]>(`/index/jobs/${id}/events`).catch(() => []),
    listJobTypes().catch(() => [] as JobTypeCatalogDto[]),
  ]);

  if (!job) {
    notFound();
  }

  const { t, locale } = await getServerTranslations();

  // One descriptor per job type: label, category, replayability, progress copy
  // and card plan all come from `lib/jobs/descriptors.ts`. The loader injected
  // here is the only server-only part (it performs the report fetches).
  const descriptor = resolveJobTypeDescriptor(job.type, {
    t,
    catalog: jobTypeCatalog,
    loaders: jobReportLoaders,
  });
  const report = descriptor.report ? await descriptor.report.load(id) : null;
  const summary = jobSummaryReports(report);

  const durationMs = job.started_at
    ? new Date(job.finished_at || new Date()).getTime() - new Date(job.started_at).getTime()
    : 0;

  const isCompleted = isCompletedJobStatus(job.status);
  const isPartial = isPartialJobStatus(job.status);
  const isTerminal = isTerminalJobStatus(job.status);

  const progress = resolveJobProgressCopy(descriptor, job.status, t);
  const { resultCard } = descriptor;

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
        batchReport={summary.batchReport}
        refreshReport={summary.refreshReport}
        readingStatusReport={summary.readingStatusReport}
        readingStatusPushReport={summary.readingStatusPushReport}
        downloadDetectionReport={summary.downloadDetectionReport}
        t={t}
        formatDuration={formatDuration}
      />

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <JobOverviewCard job={job} typeInfo={descriptor} category={descriptor.category} t={t} formatDuration={formatDuration} />
        <JobTimelineCard job={job} isThumbnailOnly={resultCard.thumbnailOnly} t={t} locale={locale} formatDuration={formatDuration} />

        {resultCard.showProgress && (
          <JobProgressCard
            job={job}
            isThumbnailOnly={resultCard.thumbnailOnly}
            progressTitle={progress.title}
            progressDescription={progress.description}
            t={t}
            formatDuration={formatDuration}
            formatSpeed={formatRate}
          />
        )}

        {/* Index statistics */}
        {resultCard.showIndexStats && job.stats_json && (
          <IndexStatsCard job={job} t={t} formatDuration={formatDuration} formatSpeed={formatRate} durationMs={durationMs} />
        )}

        {/* Thumbnail statistics */}
        {resultCard.thumbnailOnly && (isCompleted || isPartial) && job.total_files != null && (
          <ThumbnailStatsCard job={job} t={t} formatDuration={formatDuration} formatSpeed={formatRate} durationMs={durationMs} />
        )}

        {/* Report / results cards for this job type */}
        <JobResultCards descriptor={descriptor} report={report} job={job} t={t} />

        {/* Job events */}
        <JobEventsCard events={events} t={t} locale={locale} />

        {/* File errors */}
        <JobErrorsCard errors={errors} t={t} locale={locale} />

        {/* RSS snapshot — flux brut Prowlarr, disponible pour les 5 derniers jobs seulement */}
        {resultCard.kind === "rss_poll" && job.stats_json?.rss_releases && job.stats_json.rss_releases.length > 0 && (
          <RssSnapshotCard releases={job.stats_json.rss_releases} t={t} />
        )}
      </div>
    </>
  );
}

import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { jobReportOf, type JobReport, type JobTypeDescriptor } from "@/lib/jobs/descriptors";

import { MetadataBatchQuickMatch } from "./MetadataBatchQuickMatch";
import {
  MetadataBatchReportCard,
  MetadataRefreshChangesCard,
  MetadataRefreshReportCard,
} from "./MetadataReportCards";
import {
  ReadingStatusMatchReportCard,
  ReadingStatusMatchResultsCard,
  ReadingStatusPushReportCard,
  ReadingStatusPushResultsCard,
} from "./ReadingStatusReportCards";
import { DownloadDetectionErrorsCard, DownloadDetectionReportCard } from "./DownloadDetectionCards";
import { DownloadDetectionAvailableResults } from "./DownloadDetectionAvailableResults";
import { TelegramSyncResultsCard } from "./TelegramSyncResultsCard";
import { TelegramIncrementalResultsCard } from "./TelegramIncrementalResultsCard";

interface JobResultCardsJob {
  library_id: string | null;
  library_name: string | null;
  stats_json: {
    new_books?: number;
    series_searched?: number;
    all_series?: Array<{ series_name: string; book_count: number; extracted_names?: string[] }>;
    matched_series?: Array<{
      telegram_name: string;
      series_id: string;
      series_name: string;
      book_count: number;
    }>;
    sources_scanned?: number;
    sources?: Array<{
      username: string;
      new_books: number;
      messages_scanned?: number;
      error?: string;
      persistence_errors?: number;
    }>;
    recent_books?: Array<{
      filename: string;
      series_name: string | null;
      volume_number: number | null;
      channel: string;
    }>;
  } | null;
}

interface JobResultCardsProps {
  descriptor: JobTypeDescriptor;
  report: JobReport | null;
  job: JobResultCardsJob;
  t: TranslateFunction;
}

/**
 * Result cards of a job detail page, selected by the descriptor's card family.
 * Adding a report-backed job type means adding a descriptor entry and (if the
 * shape is new) a case here — never another boolean branch in the page.
 */
export function JobResultCards({ descriptor, report, job, t }: JobResultCardsProps) {
  const kind = descriptor.resultCard.kind;

  if (kind === "metadata_batch") {
    const loaded = jobReportOf(report, "metadata_batch");
    return (
      <>
        {loaded?.report && <MetadataBatchReportCard report={loaded.report} t={t} />}
        <MetadataBatchQuickMatch results={loaded?.results ?? []} libraryId={job.library_id} />
      </>
    );
  }

  if (kind === "metadata_refresh") {
    const loaded = jobReportOf(report, "metadata_refresh");
    return (
      <>
        {loaded?.report && <MetadataRefreshReportCard report={loaded.report} t={t} />}
        {loaded?.report && (
          <MetadataRefreshChangesCard report={loaded.report} libraryId={job.library_id} t={t} />
        )}
      </>
    );
  }

  if (kind === "reading_status_match") {
    const loaded = jobReportOf(report, "reading_status_match");
    return (
      <>
        {loaded?.report && <ReadingStatusMatchReportCard report={loaded.report} t={t} />}
        <ReadingStatusMatchResultsCard
          results={loaded?.results ?? []}
          libraryId={job.library_id}
          t={t}
        />
      </>
    );
  }

  if (kind === "reading_status_push") {
    const loaded = jobReportOf(report, "reading_status_push");
    return (
      <>
        {loaded?.report && <ReadingStatusPushReportCard report={loaded.report} t={t} />}
        <ReadingStatusPushResultsCard
          results={loaded?.results ?? []}
          libraryId={job.library_id}
          t={t}
        />
      </>
    );
  }

  if (kind === "download_detection" || kind === "rss_poll") {
    // Detection and Prowlarr RSS share their events and endpoints.
    const loaded = jobReportOf(report, kind);
    const results = loaded?.results ?? [];
    return (
      <>
        {loaded?.report && <DownloadDetectionReportCard report={loaded.report} t={t} />}
        <DownloadDetectionErrorsCard results={loaded?.errors ?? []} t={t} />
        {results.length > 0 && (
          <div className="lg:col-span-2">
            <DownloadDetectionAvailableResults
              results={results}
              libraryId={job.library_id || ""}
              libraryName={job.library_name || ""}
            />
          </div>
        )}
      </>
    );
  }

  if (kind === "telegram_sync") {
    if (!job.stats_json) return null;
    return (
      <TelegramSyncResultsCard
        new_books={job.stats_json.new_books ?? 0}
        series_searched={job.stats_json.series_searched ?? 0}
        all_series={job.stats_json.all_series ?? []}
        matched_series={job.stats_json.matched_series ?? []}
        t={t}
      />
    );
  }

  if (kind === "telegram_sync_incremental") {
    if (!job.stats_json) return null;
    return (
      <TelegramIncrementalResultsCard
        new_books={job.stats_json.new_books ?? 0}
        sources_scanned={job.stats_json.sources_scanned ?? 0}
        sources={job.stats_json.sources ?? []}
        recent_books={job.stats_json.recent_books ?? []}
        t={t}
      />
    );
  }

  return null;
}

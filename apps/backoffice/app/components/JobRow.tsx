"use client";

import { useState } from "react";
import Link from "next/link";
import { useTranslation } from "../../lib/i18n/context";
import { JobProgress } from "./JobProgress";
import { StatusBadge, JobTypeBadge, Button, MiniProgressBar, Icon, Tooltip } from "./ui";

interface JobRowProps {
  job: {
    id: string;
    library_id: string | null;
    type: string;
    status: string;
    created_at: string;
    started_at: string | null;
    finished_at: string | null;
    error_opt: string | null;
    stats_json: {
      scanned_files: number;
      indexed_files: number;
      removed_files: number;
      errors: number;
      refreshed?: number;
      linked?: number;
      pushed?: number;
    } | null;
    progress_percent: number | null;
    processed_files: number | null;
    total_files: number | null;
  };
  libraryName: string | undefined;
  highlighted?: boolean;
  onCancel: (id: string) => void;
  onReplay: (id: string) => void;
  formatDate: (date: string) => string;
  formatDuration: (start: string, end: string | null) => string;
}

const REPLAYABLE_TYPES = new Set(["rebuild", "full_rebuild", "rescan", "scan", "thumbnail_rebuild", "thumbnail_regenerate", "metadata_batch", "metadata_refresh", "reading_status_match", "reading_status_push"]);

export function JobRow({ job, libraryName, highlighted, onCancel, onReplay, formatDate, formatDuration }: JobRowProps) {
  const { t } = useTranslation();
  const isActive = job.status === "running" || job.status === "pending" || job.status === "extracting_pages" || job.status === "generating_thumbnails";
  const [showProgress, setShowProgress] = useState(highlighted || isActive);

  const handleComplete = () => {
    setShowProgress(false);
    window.location.reload();
  };

  // Calculate duration
  const duration = job.started_at 
    ? formatDuration(job.started_at, job.finished_at)
    : "-";

  // Get file stats
  const scanned = job.stats_json?.scanned_files ?? 0;
  const indexed = job.stats_json?.indexed_files ?? 0;
  const removed = job.stats_json?.removed_files ?? 0;
  const errors = job.stats_json?.errors ?? 0;

  const isPhase2 = job.status === "extracting_pages" || job.status === "generating_thumbnails";
  const isThumbnailPhase = job.status === "generating_thumbnails";
  const isThumbnailJob = job.type === "thumbnail_rebuild" || job.type === "thumbnail_regenerate";
  const hasThumbnailPhase = isPhase2 || isThumbnailJob;

  const isMetadataBatch = job.type === "metadata_batch";
  const isMetadataRefresh = job.type === "metadata_refresh";
  const isReadingStatusMatch = job.type === "reading_status_match";
  const isReadingStatusPush = job.type === "reading_status_push";

  // Thumbnails progress (Phase 2: extracting_pages + generating_thumbnails)
  const thumbInProgress = hasThumbnailPhase && (job.status === "running" || isPhase2);

  return (
    <>
      <tr className={highlighted ? 'bg-primary/10' : 'hover:bg-muted/50'}>
        <td className="px-4 py-3">
          <Link 
            href={`/jobs/${job.id}`} 
            className="text-primary hover:text-primary/80 hover:underline font-mono text-sm"
          >
            <code>{job.id.slice(0, 8)}</code>
          </Link>
        </td>
        <td className="px-4 py-3 text-sm text-foreground">
          {job.library_id ? libraryName || job.library_id.slice(0, 8) : "—"}
        </td>
        <td className="px-4 py-3">
          <JobTypeBadge type={job.type} />
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2 flex-wrap">
            <StatusBadge status={job.status} />
            {job.error_opt && (
              <span 
                className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-error text-white text-xs font-bold cursor-help"
                title={job.error_opt}
              >
                !
              </span>
            )}
            {isActive && (
              <button 
                className="text-xs text-primary hover:text-primary/80 hover:underline"
                onClick={() => setShowProgress(!showProgress)}
              >
                {showProgress ? t("jobRow.hideProgress") : t("jobRow.showProgress")}
              </button>
            )}
          </div>
        </td>
        <td className="px-4 py-3">
          <div className="flex flex-col gap-1">
            {/* Running progress */}
            {isActive && job.total_files != null && (
              <div className="flex flex-col gap-1">
                <span className="text-sm text-foreground">{job.processed_files ?? 0}/{job.total_files}</span>
                <MiniProgressBar value={job.processed_files ?? 0} max={job.total_files} className="w-24" />
              </div>
            )}
            {/* Completed stats with icons */}
            {!isActive && (
              <div className="flex items-center gap-3 text-xs">
                {/* Files: indexed count */}
                {indexed > 0 && (
                  <Tooltip label={t("jobRow.filesIndexed", { count: indexed })}>
                    <span className="inline-flex items-center gap-1 text-success">
                      <Icon name="document" size="sm" />
                      {indexed}
                    </span>
                  </Tooltip>
                )}
                {/* Removed files */}
                {removed > 0 && (
                  <Tooltip label={t("jobRow.filesRemoved", { count: removed })}>
                    <span className="inline-flex items-center gap-1 text-warning">
                      <Icon name="trash" size="sm" />
                      {removed}
                    </span>
                  </Tooltip>
                )}
                {/* Thumbnails */}
                {hasThumbnailPhase && job.total_files != null && job.total_files > 0 && (
                  <Tooltip label={t("jobRow.thumbnailsGenerated", { count: job.total_files })}>
                    <span className="inline-flex items-center gap-1 text-primary">
                      <Icon name="image" size="sm" />
                      {job.total_files}
                    </span>
                  </Tooltip>
                )}
                {/* Metadata batch: series processed */}
                {isMetadataBatch && job.total_files != null && job.total_files > 0 && (
                  <Tooltip label={t("jobRow.metadataProcessed", { count: job.total_files })}>
                    <span className="inline-flex items-center gap-1 text-info">
                      <Icon name="tag" size="sm" />
                      {job.total_files}
                    </span>
                  </Tooltip>
                )}
                {/* Metadata refresh: total links + refreshed count */}
                {isMetadataRefresh && job.total_files != null && job.total_files > 0 && (
                  <Tooltip label={t("jobRow.metadataLinks", { count: job.total_files })}>
                    <span className="inline-flex items-center gap-1 text-info">
                      <Icon name="tag" size="sm" />
                      {job.total_files}
                    </span>
                  </Tooltip>
                )}
                {isMetadataRefresh && job.stats_json?.refreshed != null && job.stats_json.refreshed > 0 && (
                  <Tooltip label={t("jobRow.metadataRefreshed", { count: job.stats_json.refreshed })}>
                    <span className="inline-flex items-center gap-1 text-success">
                      <Icon name="refresh" size="sm" />
                      {job.stats_json.refreshed}
                    </span>
                  </Tooltip>
                )}
                {/* Reading status match: series linked */}
                {isReadingStatusMatch && job.total_files != null && job.total_files > 0 && (
                  <Tooltip label={t("jobRow.seriesTotal", { count: job.total_files })}>
                    <span className="inline-flex items-center gap-1 text-info">
                      <Icon name="series" size="sm" />
                      {job.total_files}
                    </span>
                  </Tooltip>
                )}
                {isReadingStatusMatch && job.stats_json?.linked != null && job.stats_json.linked > 0 && (
                  <Tooltip label={t("jobRow.seriesLinked", { count: job.stats_json.linked })}>
                    <span className="inline-flex items-center gap-1 text-success">
                      <Icon name="link" size="sm" />
                      {job.stats_json.linked}
                    </span>
                  </Tooltip>
                )}
                {/* Reading status push: series pushed */}
                {isReadingStatusPush && job.total_files != null && job.total_files > 0 && (
                  <Tooltip label={t("jobRow.seriesTotal", { count: job.total_files })}>
                    <span className="inline-flex items-center gap-1 text-info">
                      <Icon name="series" size="sm" />
                      {job.total_files}
                    </span>
                  </Tooltip>
                )}
                {isReadingStatusPush && job.stats_json?.pushed != null && job.stats_json.pushed > 0 && (
                  <Tooltip label={t("jobRow.seriesPushed", { count: job.stats_json.pushed })}>
                    <span className="inline-flex items-center gap-1 text-success">
                      <Icon name="refresh" size="sm" />
                      {job.stats_json.pushed}
                    </span>
                  </Tooltip>
                )}
                {/* Errors */}
                {errors > 0 && (
                  <Tooltip label={t("jobRow.errors", { count: errors })}>
                    <span className="inline-flex items-center gap-1 text-error">
                      <Icon name="warning" size="sm" />
                      {errors}
                    </span>
                  </Tooltip>
                )}
                {/* Scanned only (no other stats) */}
                {indexed === 0 && removed === 0 && errors === 0 && !hasThumbnailPhase && !isMetadataBatch && !isMetadataRefresh && !isReadingStatusMatch && !isReadingStatusPush && scanned > 0 && (
                  <Tooltip label={t("jobRow.scanned", { count: scanned })}>
                    <span className="inline-flex items-center gap-1 text-muted-foreground">
                      <Icon name="search" size="sm" />
                      {scanned}
                    </span>
                  </Tooltip>
                )}
                {/* Nothing to show */}
                {indexed === 0 && removed === 0 && errors === 0 && scanned === 0 && !hasThumbnailPhase && !isMetadataBatch && !isMetadataRefresh && !isReadingStatusMatch && !isReadingStatusPush && (
                  <span className="text-sm text-muted-foreground">—</span>
                )}
              </div>
            )}
          </div>
        </td>
        <td className="px-4 py-3 text-sm text-muted-foreground">
          {duration}
        </td>
        <td className="px-4 py-3 text-sm text-muted-foreground">
          {formatDate(job.created_at)}
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            <Link
              href={`/jobs/${job.id}`}
              className="inline-flex items-center justify-center gap-1.5 h-7 px-2.5 text-xs font-medium rounded-md bg-primary text-white hover:bg-primary/90 transition-colors"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
              </svg>
              {t("jobRow.view")}
            </Link>
            {isActive && (
              <Button
                variant="danger"
                size="xs"
                onClick={() => onCancel(job.id)}
              >
                <svg className="w-3.5 h-3.5 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
                {t("common.cancel")}
              </Button>
            )}
            {!isActive && REPLAYABLE_TYPES.has(job.type) && (
              <Button
                variant="secondary"
                size="xs"
                onClick={() => onReplay(job.id)}
              >
                <svg className="w-3.5 h-3.5 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                {t("jobRow.replay")}
              </Button>
            )}
          </div>
        </td>
      </tr>
      {showProgress && isActive && (
        <tr>
          <td colSpan={8} className="px-4 py-3 bg-muted/50">
            <JobProgress 
              jobId={job.id}
              onComplete={handleComplete}
            />
          </td>
        </tr>
      )}
    </>
  );
}

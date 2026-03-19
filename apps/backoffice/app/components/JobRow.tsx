"use client";

import { useState } from "react";
import Link from "next/link";
import { useTranslation } from "../../lib/i18n/context";
import { JobProgress } from "./JobProgress";
import { StatusBadge, JobTypeBadge, Button, MiniProgressBar, Icon } from "./ui";

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
    } | null;
    progress_percent: number | null;
    processed_files: number | null;
    total_files: number | null;
  };
  libraryName: string | undefined;
  highlighted?: boolean;
  onCancel: (id: string) => void;
  formatDate: (date: string) => string;
  formatDuration: (start: string, end: string | null) => string;
}

export function JobRow({ job, libraryName, highlighted, onCancel, formatDate, formatDuration }: JobRowProps) {
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
                  <span className="inline-flex items-center gap-1 text-success" title={t("jobRow.filesIndexed", { count: indexed })}>
                    <Icon name="document" size="sm" />
                    {indexed}
                  </span>
                )}
                {/* Removed files */}
                {removed > 0 && (
                  <span className="inline-flex items-center gap-1 text-warning" title={t("jobRow.filesRemoved", { count: removed })}>
                    <Icon name="trash" size="sm" />
                    {removed}
                  </span>
                )}
                {/* Thumbnails */}
                {hasThumbnailPhase && job.total_files != null && job.total_files > 0 && (
                  <span className="inline-flex items-center gap-1 text-primary" title={t("jobRow.thumbnailsGenerated", { count: job.total_files })}>
                    <Icon name="image" size="sm" />
                    {job.total_files}
                  </span>
                )}
                {/* Metadata batch: series processed */}
                {isMetadataBatch && job.total_files != null && job.total_files > 0 && (
                  <span className="inline-flex items-center gap-1 text-info" title={t("jobRow.metadataProcessed", { count: job.total_files })}>
                    <Icon name="tag" size="sm" />
                    {job.total_files}
                  </span>
                )}
                {/* Metadata refresh: links refreshed */}
                {isMetadataRefresh && job.total_files != null && job.total_files > 0 && (
                  <span className="inline-flex items-center gap-1 text-info" title={t("jobRow.metadataRefreshed", { count: job.total_files })}>
                    <Icon name="tag" size="sm" />
                    {job.total_files}
                  </span>
                )}
                {/* Errors */}
                {errors > 0 && (
                  <span className="inline-flex items-center gap-1 text-error" title={t("jobRow.errors", { count: errors })}>
                    <Icon name="warning" size="sm" />
                    {errors}
                  </span>
                )}
                {/* Scanned only (no other stats) */}
                {indexed === 0 && removed === 0 && errors === 0 && !hasThumbnailPhase && !isMetadataBatch && !isMetadataRefresh && scanned > 0 && (
                  <span className="text-sm text-muted-foreground">{t("jobRow.scanned", { count: scanned })}</span>
                )}
                {/* Nothing to show */}
                {indexed === 0 && removed === 0 && errors === 0 && scanned === 0 && !hasThumbnailPhase && !isMetadataBatch && !isMetadataRefresh && (
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
              className="inline-flex items-center px-3 py-1.5 text-xs font-medium rounded-lg bg-primary text-white hover:bg-primary/90 transition-colors"
            >
              {t("jobRow.view")}
            </Link>
            {(job.status === "pending" || job.status === "running" || job.status === "extracting_pages" || job.status === "generating_thumbnails") && (
              <Button 
                variant="danger" 
                size="sm"
                onClick={() => onCancel(job.id)}
              >
                {t("common.cancel")}
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

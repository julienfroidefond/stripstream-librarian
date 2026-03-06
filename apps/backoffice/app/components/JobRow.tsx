"use client";

import { useState } from "react";
import Link from "next/link";
import { JobProgress } from "./JobProgress";
import { StatusBadge, Button, MiniProgressBar } from "./ui";

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
  const [showProgress, setShowProgress] = useState(
    highlighted || job.status === "running" || job.status === "pending"
  );

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

  // Format files display
  const filesDisplay = job.status === "running" && job.total_files
    ? `${job.processed_files || 0}/${job.total_files}`
    : scanned > 0 
      ? `${scanned} scanned`
      : "-";

  return (
    <>
      <tr className={highlighted ? 'bg-primary-soft/50' : 'hover:bg-muted/5'}>
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
        <td className="px-4 py-3 text-sm text-foreground">{job.type}</td>
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
            {(job.status === "running" || job.status === "pending") && (
              <button 
                className="text-xs text-primary hover:text-primary/80 hover:underline"
                onClick={() => setShowProgress(!showProgress)}
              >
                {showProgress ? "Hide" : "Show"} progress
              </button>
            )}
          </div>
        </td>
        <td className="px-4 py-3">
          <div className="flex flex-col gap-1">
            <span className="text-sm text-foreground">{filesDisplay}</span>
            {job.status === "running" && job.total_files && (
              <MiniProgressBar 
                value={job.processed_files || 0} 
                max={job.total_files}
                className="w-24"
              />
            )}
            {job.status === "success" && (
              <div className="flex items-center gap-2 text-xs">
                <span className="text-success">✓ {indexed}</span>
                {removed > 0 && <span className="text-warning">− {removed}</span>}
                {errors > 0 && <span className="text-error">⚠ {errors}</span>}
              </div>
            )}
          </div>
        </td>
        <td className="px-4 py-3 text-sm text-muted">
          {duration}
        </td>
        <td className="px-4 py-3 text-sm text-muted">
          {formatDate(job.created_at)}
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            <Link 
              href={`/jobs/${job.id}`}
              className="inline-flex items-center px-3 py-1.5 text-xs font-medium rounded-lg bg-primary text-white hover:bg-primary/90 transition-colors"
            >
              View
            </Link>
            {(job.status === "pending" || job.status === "running") && (
              <Button 
                variant="danger" 
                size="sm"
                onClick={() => onCancel(job.id)}
              >
                Cancel
              </Button>
            )}
          </div>
        </td>
      </tr>
      {showProgress && (job.status === "running" || job.status === "pending") && (
        <tr>
          <td colSpan={8} className="px-4 py-3 bg-muted/5">
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

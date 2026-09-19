"use client";

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../lib/i18n/context";
import { StatusBadge, Badge, ProgressBar } from "./ui";
import { isPhase2Status } from "@/lib/jobStatus";

interface ProgressEvent {
  job_id: string;
  status: string;
  current_file: string | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
  } | null;
}

interface JobProgressProps {
  jobId: string;
  onComplete?: () => void;
}

export function JobProgress({ jobId, onComplete }: JobProgressProps) {
  const { t } = useTranslation();
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isComplete, setIsComplete] = useState(false);

  // Stash mutable callbacks in refs so we don't re-open the SSE on every parent
  // render (onComplete is typically an inline arrow function in the parent,
  // and t changes shape often enough to retrigger the effect).
  const onCompleteRef = useRef(onComplete);
  onCompleteRef.current = onComplete;
  const tRef = useRef(t);
  tRef.current = t;

  useEffect(() => {
    const eventSource = new EventSource(`/api/jobs/${jobId}/stream`);

    eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);

        const progressData: ProgressEvent = {
          job_id: data.id,
          status: data.status,
          current_file: data.current_file,
          progress_percent: data.progress_percent,
          processed_files: data.processed_files,
          total_files: data.total_files,
          stats_json: data.stats_json,
        };

        setProgress(progressData);

        if (data.status === "success" || data.status === "failed" || data.status === "cancelled") {
          setIsComplete(true);
          eventSource.close();
          onCompleteRef.current?.();
        }
      } catch {
        setError(tRef.current("jobProgress.sseError"));
      }
    };

    eventSource.onerror = (err) => {
      console.error("SSE error:", err);
      eventSource.close();
      setError(tRef.current("jobProgress.connectionLost"));
    };

    return () => {
      eventSource.close();
    };
  }, [jobId]);

  if (error) {
    return (
      <div className="p-4 bg-destructive/10 text-error rounded-lg text-sm">
        {t("jobProgress.error", { message: error })}
      </div>
    );
  }

  if (!progress) {
    return (
      <div className="p-4 text-muted-foreground text-sm">
        {t("jobProgress.loadingProgress")}
      </div>
    );
  }

  const percent = progress.progress_percent ?? 0;
  const processed = progress.processed_files ?? 0;
  const total = progress.total_files ?? 0;
  const isPhase2 = isPhase2Status(progress.status);
  const unitLabel = progress.status === "extracting_pages" ? t("jobProgress.pages") : progress.status === "generating_thumbnails" ? t("jobProgress.thumbnails") : t("jobProgress.filesUnit");

  return (
    <div className="p-4 bg-card rounded-lg border border-border">
      <div className="flex items-center justify-between mb-3">
        <StatusBadge status={progress.status} />
        {isComplete && (
          <Badge variant="success">{t("jobProgress.done")}</Badge>
        )}
      </div>

      <ProgressBar value={percent} showLabel size="lg" className="mb-3" />

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground mb-3">
        <span>{processed} / {total} {unitLabel}</span>
        {progress.current_file && (
          <span className="truncate max-w-md" title={progress.current_file}>
            {t("jobProgress.currentFile", { file: progress.current_file.length > 40
              ? progress.current_file.substring(0, 40) + "..."
              : progress.current_file })}
          </span>
        )}
      </div>

      {progress.stats_json && !isPhase2 && (
        <div className="flex flex-wrap gap-3 text-xs">
          <Badge variant="primary">{t("jobProgress.scanned", { count: progress.stats_json.scanned_files })}</Badge>
          <Badge variant="success">{t("jobProgress.indexed", { count: progress.stats_json.indexed_files })}</Badge>
          <Badge variant="warning">{t("jobProgress.removed", { count: progress.stats_json.removed_files })}</Badge>
          {progress.stats_json.errors > 0 && (
            <Badge variant="error">{t("jobProgress.errors", { count: progress.stats_json.errors })}</Badge>
          )}
        </div>
      )}
    </div>
  );
}

import { Card, CardHeader, CardTitle, CardDescription, CardContent, StatBox, ProgressBar } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface JobProgressCardProps {
  job: {
    type: string;
    status: string;
    started_at: string | null;
    finished_at: string | null;
    total_files: number | null;
    processed_files: number | null;
    progress_percent: number | null;
    current_file: string | null;
    stats_json: Record<string, unknown> | null;
  };
  isThumbnailOnly: boolean;
  progressTitle: string;
  progressDescription: string | undefined;
  t: TranslateFunction;
  formatDuration: (start: string, end: string | null) => string;
  formatSpeed: (count: number, durationMs: number) => string;
}

export function JobProgressCard({ job, isThumbnailOnly, progressTitle, progressDescription, t }: JobProgressCardProps) {
  const isCompleted = job.status === "success";
  const isPhase2 = job.status === "extracting_pages" || job.status === "generating_thumbnails";

  const showProgressCard =
    (isCompleted || job.status === "failed" || job.status === "running" || isPhase2) &&
    (job.total_files != null || !!job.current_file);

  if (!showProgressCard) return null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{progressTitle}</CardTitle>
        {progressDescription && <CardDescription>{progressDescription}</CardDescription>}
      </CardHeader>
      <CardContent>
        {job.total_files != null && job.total_files > 0 && (
          <>
            <ProgressBar value={job.progress_percent || 0} showLabel size="lg" className="mb-4" />
            <div className="grid grid-cols-3 gap-4">
              <StatBox
                value={job.processed_files ?? 0}
                label={isThumbnailOnly || isPhase2 ? t("jobDetail.generated") : t("jobDetail.processed")}
                variant="primary"
              />
              <StatBox value={job.total_files} label={t("jobDetail.total")} />
              <StatBox
                value={Math.max(0, job.total_files - (job.processed_files ?? 0))}
                label={t("jobDetail.remaining")}
                variant={isCompleted ? "default" : "warning"}
              />
            </div>
          </>
        )}
        {job.current_file && (
          <div className="mt-4 p-3 bg-muted/50 rounded-lg">
            <span className="text-xs text-muted-foreground uppercase tracking-wide">{t("jobDetail.currentFile")}</span>
            <code className="block mt-1 text-xs font-mono text-foreground break-all">{job.current_file}</code>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export function IndexStatsCard({ job, t, formatDuration, formatSpeed, durationMs }: {
  job: JobProgressCardProps["job"];
  t: TranslateFunction;
  formatDuration: (start: string, end: string | null) => string;
  formatSpeed: (count: number, durationMs: number) => string;
  durationMs: number;
}) {
  const s = job.stats_json as { scanned_files?: number; indexed_files?: number; removed_files?: number; errors?: number; warnings?: number } | null;
  if (!s) return null;

  const speedCount = s.scanned_files ?? 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.indexStats")}</CardTitle>
        {job.started_at && (
          <CardDescription>
            {formatDuration(job.started_at, job.finished_at)}
            {speedCount > 0 && ` · ${formatSpeed(speedCount, durationMs)} scan rate`}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 sm:grid-cols-5 gap-4">
          <StatBox value={s.scanned_files ?? 0} label={t("jobDetail.scanned")} variant="success" />
          <StatBox value={s.indexed_files ?? 0} label={t("jobDetail.indexed")} variant="primary" />
          <StatBox value={s.removed_files ?? 0} label={t("jobDetail.removed")} variant="warning" />
          <StatBox value={s.warnings ?? 0} label={t("jobDetail.warnings")} variant={(s.warnings ?? 0) > 0 ? "warning" : "default"} />
          <StatBox value={s.errors ?? 0} label={t("jobDetail.errors")} variant={(s.errors ?? 0) > 0 ? "error" : "default"} />
        </div>
      </CardContent>
    </Card>
  );
}

export function ThumbnailStatsCard({ job, t, formatDuration, formatSpeed, durationMs }: {
  job: JobProgressCardProps["job"];
  t: TranslateFunction;
  formatDuration: (start: string, end: string | null) => string;
  formatSpeed: (count: number, durationMs: number) => string;
  durationMs: number;
}) {
  if (job.total_files == null) return null;

  const speedCount = job.processed_files ?? 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.thumbnailStats")}</CardTitle>
        {job.started_at && (
          <CardDescription>
            {formatDuration(job.started_at, job.finished_at)}
            {speedCount > 0 && ` · ${formatSpeed(speedCount, durationMs)} thumbnails/s`}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-4">
          <StatBox value={job.processed_files ?? job.total_files} label={t("jobDetail.generated")} variant="success" />
          <StatBox value={job.total_files} label={t("jobDetail.total")} />
        </div>
      </CardContent>
    </Card>
  );
}

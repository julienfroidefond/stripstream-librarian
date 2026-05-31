import { Card, CardHeader, CardTitle, CardContent } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface JobTimelineCardProps {
  job: {
    type: string;
    status: string;
    created_at: string;
    started_at: string | null;
    finished_at: string | null;
    phase2_started_at: string | null;
    generating_thumbnails_started_at: string | null;
    stats_json: Record<string, unknown> | null;
    total_files: number | null;
    processed_files: number | null;
  };
  isThumbnailOnly: boolean;
  t: TranslateFunction;
  locale: string;
  formatDuration: (start: string, end: string | null) => string;
}

export function JobTimelineCard({ job, isThumbnailOnly, t, locale, formatDuration }: JobTimelineCardProps) {
  const isCompleted = job.status === "success";
  const isFailed = job.status === "failed";
  const isExtractingPages = job.status === "extracting_pages";
  const isThumbnailPhase = job.status === "generating_thumbnails";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.timeline")}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="relative">
          <div className="absolute left-[7px] top-2 bottom-2 w-px bg-border" />
          <div className="space-y-5">
            {/* Created */}
            <div className="flex items-start gap-4">
              <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-muted border-2 border-border shrink-0 z-10" />
              <div className="flex-1 min-w-0">
                <span className="text-sm font-medium text-foreground">{t("jobDetail.created")}</span>
                <p className="text-xs text-muted-foreground">{new Date(job.created_at).toLocaleString(locale)}</p>
              </div>
            </div>

            {/* Phase 1 start */}
            {job.started_at && job.phase2_started_at && (
              <div className="flex items-start gap-4">
                <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-primary shrink-0 z-10" />
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">{t("jobDetail.phase1")}</span>
                  <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString(locale)}</p>
                  <p className="text-xs text-primary/80 font-medium mt-0.5">
                    {t("jobDetail.duration", { duration: formatDuration(job.started_at, job.phase2_started_at) })}
                    {job.stats_json && (job.stats_json as { scanned_files?: number }).scanned_files != null && (
                      <span className="text-muted-foreground font-normal ml-1">
                        {(() => {
                          const s = job.stats_json as { scanned_files?: number; indexed_files?: number; removed_files?: number; warnings?: number };
                          return <>
                            · {s.scanned_files} {t("jobDetail.scanned").toLowerCase()}, {s.indexed_files} {t("jobDetail.indexed").toLowerCase()}
                            {(s.removed_files ?? 0) > 0 && `, ${s.removed_files} ${t("jobDetail.removed").toLowerCase()}`}
                            {(s.warnings ?? 0) > 0 && `, ${s.warnings} ${t("jobDetail.warnings").toLowerCase()}`}
                          </>;
                        })()}
                      </span>
                    )}
                  </p>
                </div>
              </div>
            )}

            {/* Phase 2a — Extracting pages */}
            {job.phase2_started_at && !isThumbnailOnly && (
              <div className="flex items-start gap-4">
                <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                  job.generating_thumbnails_started_at || job.finished_at ? "bg-primary" : "bg-primary animate-pulse"
                }`} />
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">{t("jobDetail.phase2a")}</span>
                  <p className="text-xs text-muted-foreground">{new Date(job.phase2_started_at).toLocaleString(locale)}</p>
                  <p className="text-xs text-primary/80 font-medium mt-0.5">
                    {t("jobDetail.duration", { duration: formatDuration(job.phase2_started_at, job.generating_thumbnails_started_at ?? job.finished_at ?? null) })}
                    {!job.generating_thumbnails_started_at && !job.finished_at && isExtractingPages && (
                      <span className="text-muted-foreground font-normal ml-1">· {t("jobDetail.inProgress")}</span>
                    )}
                  </p>
                </div>
              </div>
            )}

            {/* Phase 2b — Generating thumbnails */}
            {(job.generating_thumbnails_started_at || (job.phase2_started_at && isThumbnailOnly)) && (
              <div className="flex items-start gap-4">
                <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                  job.finished_at ? "bg-primary" : "bg-primary animate-pulse"
                }`} />
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">
                    {isThumbnailOnly ? t("jobType.thumbnail_rebuild") : t("jobDetail.phase2b")}
                  </span>
                  <p className="text-xs text-muted-foreground">
                    {(job.generating_thumbnails_started_at ? new Date(job.generating_thumbnails_started_at) : job.phase2_started_at ? new Date(job.phase2_started_at) : null)?.toLocaleString(locale)}
                  </p>
                  {(job.generating_thumbnails_started_at || job.finished_at) && (
                    <p className="text-xs text-primary/80 font-medium mt-0.5">
                      {t("jobDetail.duration", { duration: formatDuration(
                        job.generating_thumbnails_started_at ?? job.phase2_started_at!,
                        job.finished_at ?? null
                      ) })}
                      {job.total_files != null && job.total_files > 0 && (
                        <span className="text-muted-foreground font-normal ml-1">
                          · {job.processed_files ?? job.total_files} {t("jobType.thumbnail_rebuild").toLowerCase()}
                        </span>
                      )}
                    </p>
                  )}
                  {!job.finished_at && isThumbnailPhase && (
                    <span className="text-xs text-muted-foreground">{t("jobDetail.inProgress")}</span>
                  )}
                </div>
              </div>
            )}

            {/* Started — for jobs without phase2 */}
            {job.started_at && !job.phase2_started_at && (
              <div className="flex items-start gap-4">
                <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                  job.finished_at ? "bg-primary" : "bg-primary animate-pulse"
                }`} />
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">{t("jobDetail.started")}</span>
                  <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString(locale)}</p>
                </div>
              </div>
            )}

            {/* Pending */}
            {!job.started_at && (
              <div className="flex items-start gap-4">
                <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-warning shrink-0 z-10" />
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">{t("jobDetail.pendingStart")}</span>
                </div>
              </div>
            )}

            {/* Finished */}
            {job.finished_at && (
              <div className="flex items-start gap-4">
                <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                  isCompleted ? "bg-success" : isFailed ? "bg-destructive" : "bg-muted"
                }`} />
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">
                    {isCompleted ? t("jobDetail.finished") : isFailed ? t("jobDetail.failed") : t("jobDetail.cancelled")}
                  </span>
                  <p className="text-xs text-muted-foreground">{new Date(job.finished_at).toLocaleString(locale)}</p>
                </div>
              </div>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

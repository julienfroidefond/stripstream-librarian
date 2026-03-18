import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch, getMetadataBatchReport, getMetadataBatchResults, MetadataBatchReportDto, MetadataBatchResultDto } from "../../../lib/api";
import {
  Card, CardHeader, CardTitle, CardDescription, CardContent,
  StatusBadge, JobTypeBadge, StatBox, ProgressBar
} from "../../components/ui";
import { getServerTranslations } from "../../../lib/i18n/server";

interface JobDetailPageProps {
  params: Promise<{ id: string }>;
}

interface JobDetails {
  id: string;
  library_id: string | null;
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
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
    warnings: number;
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
  const [job, errors] = await Promise.all([
    getJobDetails(id),
    getJobErrors(id),
  ]);

  if (!job) {
    notFound();
  }

  const { t, locale } = await getServerTranslations();

  const JOB_TYPE_INFO: Record<string, { label: string; description: string; isThumbnailOnly: boolean }> = {
    rebuild: {
      label: t("jobType.rebuildLabel"),
      description: t("jobType.rebuildDesc"),
      isThumbnailOnly: false,
    },
    full_rebuild: {
      label: t("jobType.full_rebuildLabel"),
      description: t("jobType.full_rebuildDesc"),
      isThumbnailOnly: false,
    },
    thumbnail_rebuild: {
      label: t("jobType.thumbnail_rebuildLabel"),
      description: t("jobType.thumbnail_rebuildDesc"),
      isThumbnailOnly: true,
    },
    thumbnail_regenerate: {
      label: t("jobType.thumbnail_regenerateLabel"),
      description: t("jobType.thumbnail_regenerateDesc"),
      isThumbnailOnly: true,
    },
    cbr_to_cbz: {
      label: t("jobType.cbr_to_cbzLabel"),
      description: t("jobType.cbr_to_cbzDesc"),
      isThumbnailOnly: false,
    },
    metadata_batch: {
      label: t("jobType.metadata_batchLabel"),
      description: t("jobType.metadata_batchDesc"),
      isThumbnailOnly: false,
    },
  };

  const isMetadataBatch = job.type === "metadata_batch";

  // Fetch batch report & results for metadata_batch jobs
  let batchReport: MetadataBatchReportDto | null = null;
  let batchResults: MetadataBatchResultDto[] = [];
  if (isMetadataBatch) {
    [batchReport, batchResults] = await Promise.all([
      getMetadataBatchReport(id).catch(() => null),
      getMetadataBatchResults(id).catch(() => []),
    ]);
  }

  const typeInfo = JOB_TYPE_INFO[job.type] ?? {
    label: job.type,
    description: null,
    isThumbnailOnly: false,
  };

  const durationMs = job.started_at
    ? new Date(job.finished_at || new Date()).getTime() - new Date(job.started_at).getTime()
    : 0;

  const isCompleted = job.status === "success";
  const isFailed = job.status === "failed";
  const isCancelled = job.status === "cancelled";
  const isExtractingPages = job.status === "extracting_pages";
  const isThumbnailPhase = job.status === "generating_thumbnails";
  const isPhase2 = isExtractingPages || isThumbnailPhase;
  const { isThumbnailOnly } = typeInfo;

  // Which label to use for the progress card
  const progressTitle = isMetadataBatch
    ? t("jobDetail.metadataSearch")
    : isThumbnailOnly
      ? t("jobType.thumbnail_rebuild")
      : isExtractingPages
        ? t("jobDetail.phase2a")
        : isThumbnailPhase
          ? t("jobDetail.phase2b")
          : t("jobDetail.phase1");

  const progressDescription = isMetadataBatch
    ? t("jobDetail.metadataSearchDesc")
    : isThumbnailOnly
      ? undefined
      : isExtractingPages
        ? t("jobDetail.phase2aDesc")
        : isThumbnailPhase
          ? t("jobDetail.phase2bDesc")
          : t("jobDetail.phase1Desc");

  // Speed metric: thumbnail count for thumbnail jobs, scanned files for index jobs
  const speedCount = isThumbnailOnly
    ? (job.processed_files ?? 0)
    : (job.stats_json?.scanned_files ?? 0);

  const showProgressCard =
    (isCompleted || isFailed || job.status === "running" || isPhase2) &&
    (job.total_files != null || !!job.current_file);

  return (
    <>
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

      {/* Summary banner — completed */}
      {isCompleted && job.started_at && (
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
            {!isMetadataBatch && job.stats_json && (
              <span className="ml-2 text-success/80">
                — {job.stats_json.scanned_files} {t("jobDetail.scanned").toLowerCase()}, {job.stats_json.indexed_files} {t("jobDetail.indexed").toLowerCase()}
                {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} ${t("jobDetail.removed").toLowerCase()}`}
                {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} ${t("jobDetail.warnings").toLowerCase()}`}
                {job.stats_json.errors > 0 && `, ${job.stats_json.errors} ${t("jobDetail.errors").toLowerCase()}`}
                {job.total_files != null && job.total_files > 0 && `, ${job.total_files} ${t("jobType.thumbnail_rebuild").toLowerCase()}`}
              </span>
            )}
            {!isMetadataBatch && !job.stats_json && isThumbnailOnly && job.total_files != null && (
              <span className="ml-2 text-success/80">
                — {job.processed_files ?? job.total_files} {t("jobDetail.generated").toLowerCase()}
              </span>
            )}
          </div>
        </div>
      )}

      {/* Summary banner — failed */}
      {isFailed && (
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
      )}

      {/* Summary banner — cancelled */}
      {isCancelled && (
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
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Overview Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t("jobDetail.overview")}</CardTitle>
            {typeInfo.description && (
              <CardDescription>{typeInfo.description}</CardDescription>
            )}
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">ID</span>
              <code className="px-2 py-1 bg-muted rounded font-mono text-sm text-foreground">{job.id}</code>
            </div>
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">{t("jobsList.type")}</span>
              <div className="flex items-center gap-2">
                <JobTypeBadge type={job.type} />
                <span className="text-sm text-muted-foreground">{typeInfo.label}</span>
              </div>
            </div>
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">{t("jobsList.status")}</span>
              <StatusBadge status={job.status} />
            </div>
            <div className={`flex items-center justify-between py-2 ${(job.book_id || job.started_at) ? "border-b border-border/60" : ""}`}>
              <span className="text-sm text-muted-foreground">{t("jobDetail.library")}</span>
              <span className="text-sm text-foreground">{job.library_id || t("jobDetail.allLibraries")}</span>
            </div>
            {job.book_id && (
              <div className={`flex items-center justify-between py-2 ${job.started_at ? "border-b border-border/60" : ""}`}>
                <span className="text-sm text-muted-foreground">{t("jobDetail.book")}</span>
                <Link
                  href={`/books/${job.book_id}`}
                  className="text-sm text-primary hover:text-primary/80 font-mono hover:underline"
                >
                  {job.book_id.slice(0, 8)}…
                </Link>
              </div>
            )}
            {job.started_at && (
              <div className="flex items-center justify-between py-2">
                <span className="text-sm text-muted-foreground">{t("jobsList.duration")}</span>
                <span className="text-sm font-semibold text-foreground">
                  {formatDuration(job.started_at, job.finished_at)}
                </span>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Timeline Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t("jobDetail.timeline")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="relative">
              {/* Vertical line */}
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

                {/* Phase 1 start — for index jobs that have two phases */}
                {job.started_at && job.phase2_started_at && (
                  <div className="flex items-start gap-4">
                    <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-primary shrink-0 z-10" />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">{t("jobDetail.phase1")}</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString(locale)}</p>
                      <p className="text-xs text-primary/80 font-medium mt-0.5">
                        {t("jobDetail.duration", { duration: formatDuration(job.started_at, job.phase2_started_at) })}
                        {job.stats_json && (
                          <span className="text-muted-foreground font-normal ml-1">
                            · {job.stats_json.scanned_files} {t("jobDetail.scanned").toLowerCase()}, {job.stats_json.indexed_files} {t("jobDetail.indexed").toLowerCase()}
                            {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} ${t("jobDetail.removed").toLowerCase()}`}
                            {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} ${t("jobDetail.warnings").toLowerCase()}`}
                          </span>
                        )}
                      </p>
                    </div>
                  </div>
                )}

                {/* Phase 2a — Extracting pages (index jobs with phase2) */}
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

                {/* Started — for jobs without phase2 (cbr_to_cbz, or no phase yet) */}
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

                {/* Pending — not started yet */}
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

        {/* Progress Card */}
        {showProgressCard && (
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
        )}

        {/* Index Statistics — index jobs only */}
        {job.stats_json && !isThumbnailOnly && !isMetadataBatch && (
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
                <StatBox value={job.stats_json.scanned_files} label={t("jobDetail.scanned")} variant="success" />
                <StatBox value={job.stats_json.indexed_files} label={t("jobDetail.indexed")} variant="primary" />
                <StatBox value={job.stats_json.removed_files} label={t("jobDetail.removed")} variant="warning" />
                <StatBox value={job.stats_json.warnings ?? 0} label={t("jobDetail.warnings")} variant={(job.stats_json.warnings ?? 0) > 0 ? "warning" : "default"} />
                <StatBox value={job.stats_json.errors} label={t("jobDetail.errors")} variant={job.stats_json.errors > 0 ? "error" : "default"} />
              </div>
            </CardContent>
          </Card>
        )}

        {/* Thumbnail statistics — thumbnail-only jobs, completed */}
        {isThumbnailOnly && isCompleted && job.total_files != null && (
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
        )}

        {/* Metadata batch report */}
        {isMetadataBatch && batchReport && (
          <Card>
            <CardHeader>
              <CardTitle>{t("jobDetail.batchReport")}</CardTitle>
              <CardDescription>{t("jobDetail.seriesAnalyzed", { count: String(batchReport.total_series) })}</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
                <StatBox value={batchReport.auto_matched} label={t("jobDetail.autoMatched")} variant="success" />
                <StatBox value={batchReport.already_linked} label={t("jobDetail.alreadyLinked")} variant="primary" />
                <StatBox value={batchReport.no_results} label={t("jobDetail.noResults")} />
                <StatBox value={batchReport.too_many_results} label={t("jobDetail.tooManyResults")} variant="warning" />
                <StatBox value={batchReport.low_confidence} label={t("jobDetail.lowConfidence")} variant="warning" />
                <StatBox value={batchReport.errors} label={t("jobDetail.errors")} variant={batchReport.errors > 0 ? "error" : "default"} />
              </div>
            </CardContent>
          </Card>
        )}

        {/* Metadata batch results */}
        {isMetadataBatch && batchResults.length > 0 && (
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>{t("jobDetail.resultsBySeries")}</CardTitle>
              <CardDescription>{t("jobDetail.seriesProcessed", { count: String(batchResults.length) })}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-2 max-h-[600px] overflow-y-auto">
              {batchResults.map((r) => (
                <div
                  key={r.id}
                  className={`p-3 rounded-lg border ${
                    r.status === "auto_matched" ? "bg-success/10 border-success/20" :
                    r.status === "already_linked" ? "bg-primary/10 border-primary/20" :
                    r.status === "error" ? "bg-destructive/10 border-destructive/20" :
                    "bg-muted/50 border-border/60"
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-sm text-foreground truncate">{r.series_name}</span>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap ${
                      r.status === "auto_matched" ? "bg-success/20 text-success" :
                      r.status === "already_linked" ? "bg-primary/20 text-primary" :
                      r.status === "no_results" ? "bg-muted text-muted-foreground" :
                      r.status === "too_many_results" ? "bg-amber-500/15 text-amber-600" :
                      r.status === "low_confidence" ? "bg-amber-500/15 text-amber-600" :
                      r.status === "error" ? "bg-destructive/20 text-destructive" :
                      "bg-muted text-muted-foreground"
                    }`}>
                      {r.status === "auto_matched" ? t("jobDetail.autoMatched") :
                       r.status === "already_linked" ? t("jobDetail.alreadyLinked") :
                       r.status === "no_results" ? t("jobDetail.noResults") :
                       r.status === "too_many_results" ? t("jobDetail.tooManyResults") :
                       r.status === "low_confidence" ? t("jobDetail.lowConfidence") :
                       r.status === "error" ? t("common.error") :
                       r.status}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                    {r.provider_used && (
                      <span>{r.provider_used}{r.fallback_used ? ` ${t("metadata.fallbackUsed")}` : ""}</span>
                    )}
                    {r.candidates_count > 0 && (
                      <span>{r.candidates_count} {t("jobDetail.candidates", { plural: r.candidates_count > 1 ? "s" : "" })}</span>
                    )}
                    {r.best_confidence != null && (
                      <span>{Math.round(r.best_confidence * 100)}% {t("jobDetail.confidence")}</span>
                    )}
                  </div>
                  {r.best_candidate_json && (
                    <p className="text-xs text-muted-foreground mt-1">
                      {t("jobDetail.match", { title: (r.best_candidate_json as { title?: string }).title || r.best_candidate_json.toString() })}
                    </p>
                  )}
                  {r.error_message && (
                    <p className="text-xs text-destructive/80 mt-1">{r.error_message}</p>
                  )}
                </div>
              ))}
            </CardContent>
          </Card>
        )}

        {/* File errors */}
        {errors.length > 0 && (
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>{t("jobDetail.fileErrors", { count: String(errors.length) })}</CardTitle>
              <CardDescription>{t("jobDetail.fileErrorsDesc")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-2 max-h-80 overflow-y-auto">
              {errors.map((error) => (
                <div key={error.id} className="p-3 bg-destructive/10 rounded-lg border border-destructive/20">
                  <code className="block text-sm font-mono text-destructive mb-1">{error.file_path}</code>
                  <p className="text-sm text-destructive/80">{error.error_message}</p>
                  <span className="text-xs text-muted-foreground">{new Date(error.created_at).toLocaleString(locale)}</span>
                </div>
              ))}
            </CardContent>
          </Card>
        )}
      </div>
    </>
  );
}

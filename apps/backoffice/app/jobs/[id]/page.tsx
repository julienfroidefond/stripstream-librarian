import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch } from "../../../lib/api";
import {
  Card, CardHeader, CardTitle, CardDescription, CardContent,
  StatusBadge, JobTypeBadge, StatBox, ProgressBar
} from "../../components/ui";

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

const JOB_TYPE_INFO: Record<string, { label: string; description: string; isThumbnailOnly: boolean }> = {
  rebuild: {
    label: "Incremental index",
    description: "Scans for new/modified files, analyzes them and generates missing thumbnails.",
    isThumbnailOnly: false,
  },
  full_rebuild: {
    label: "Full re-index",
    description: "Clears all existing data then performs a complete re-scan, re-analysis and thumbnail generation.",
    isThumbnailOnly: false,
  },
  thumbnail_rebuild: {
    label: "Thumbnail rebuild",
    description: "Generates thumbnails only for books that are missing one. Existing thumbnails are preserved.",
    isThumbnailOnly: true,
  },
  thumbnail_regenerate: {
    label: "Thumbnail regeneration",
    description: "Regenerates all thumbnails from scratch, replacing existing ones.",
    isThumbnailOnly: true,
  },
  cbr_to_cbz: {
    label: "CBR → CBZ conversion",
    description: "Converts a CBR archive to the open CBZ format.",
    isThumbnailOnly: false,
  },
};

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
  const progressTitle = isThumbnailOnly
    ? "Thumbnails"
    : isExtractingPages
      ? "Phase 2 — Extracting pages"
      : isThumbnailPhase
        ? "Phase 2 — Thumbnails"
        : "Phase 1 — Discovery";

  const progressDescription = isThumbnailOnly
    ? undefined
    : isExtractingPages
      ? "Extracting first page from each archive (page count + raw image)"
      : isThumbnailPhase
        ? "Generating thumbnails for the analyzed books"
        : "Scanning and indexing files in the library";

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
          Back to jobs
        </Link>
        <h1 className="text-3xl font-bold text-foreground mt-2">Job Details</h1>
      </div>

      {/* Summary banner — completed */}
      {isCompleted && job.started_at && (
        <div className="mb-6 p-4 rounded-xl bg-success/10 border border-success/30 flex items-start gap-3">
          <svg className="w-5 h-5 text-success mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div className="text-sm text-success">
            <span className="font-semibold">Completed in {formatDuration(job.started_at, job.finished_at)}</span>
            {job.stats_json && (
              <span className="ml-2 text-success/80">
                — {job.stats_json.scanned_files} scanned, {job.stats_json.indexed_files} indexed
                {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} removed`}
                {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} warnings`}
                {job.stats_json.errors > 0 && `, ${job.stats_json.errors} errors`}
                {job.total_files != null && job.total_files > 0 && `, ${job.total_files} thumbnails`}
              </span>
            )}
            {!job.stats_json && isThumbnailOnly && job.total_files != null && (
              <span className="ml-2 text-success/80">
                — {job.processed_files ?? job.total_files} thumbnails generated
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
            <span className="font-semibold">Job failed</span>
            {job.started_at && (
              <span className="ml-2 text-destructive/80">after {formatDuration(job.started_at, job.finished_at)}</span>
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
            <span className="font-semibold">Cancelled</span>
            {job.started_at && (
              <span className="ml-2">after {formatDuration(job.started_at, job.finished_at)}</span>
            )}
          </span>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Overview Card */}
        <Card>
          <CardHeader>
            <CardTitle>Overview</CardTitle>
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
              <span className="text-sm text-muted-foreground">Type</span>
              <div className="flex items-center gap-2">
                <JobTypeBadge type={job.type} />
                <span className="text-sm text-muted-foreground">{typeInfo.label}</span>
              </div>
            </div>
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">Status</span>
              <StatusBadge status={job.status} />
            </div>
            <div className={`flex items-center justify-between py-2 ${(job.book_id || job.started_at) ? "border-b border-border/60" : ""}`}>
              <span className="text-sm text-muted-foreground">Library</span>
              <span className="text-sm text-foreground">{job.library_id || "All libraries"}</span>
            </div>
            {job.book_id && (
              <div className={`flex items-center justify-between py-2 ${job.started_at ? "border-b border-border/60" : ""}`}>
                <span className="text-sm text-muted-foreground">Book</span>
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
                <span className="text-sm text-muted-foreground">Duration</span>
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
            <CardTitle>Timeline</CardTitle>
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
                    <span className="text-sm font-medium text-foreground">Created</span>
                    <p className="text-xs text-muted-foreground">{new Date(job.created_at).toLocaleString()}</p>
                  </div>
                </div>

                {/* Phase 1 start — for index jobs that have two phases */}
                {job.started_at && job.phase2_started_at && (
                  <div className="flex items-start gap-4">
                    <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-primary shrink-0 z-10" />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">Phase 1 — Discovery</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString()}</p>
                      <p className="text-xs text-primary/80 font-medium mt-0.5">
                        Duration: {formatDuration(job.started_at, job.phase2_started_at)}
                        {job.stats_json && (
                          <span className="text-muted-foreground font-normal ml-1">
                            · {job.stats_json.scanned_files} scanned, {job.stats_json.indexed_files} indexed
                            {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} removed`}
                            {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} warn`}
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
                      <span className="text-sm font-medium text-foreground">Phase 2a — Extracting pages</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.phase2_started_at).toLocaleString()}</p>
                      <p className="text-xs text-primary/80 font-medium mt-0.5">
                        Duration: {formatDuration(job.phase2_started_at, job.generating_thumbnails_started_at ?? job.finished_at ?? null)}
                        {!job.generating_thumbnails_started_at && !job.finished_at && isExtractingPages && (
                          <span className="text-muted-foreground font-normal ml-1">· in progress</span>
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
                        {isThumbnailOnly ? "Thumbnails" : "Phase 2b — Generating thumbnails"}
                      </span>
                      <p className="text-xs text-muted-foreground">
                        {(job.generating_thumbnails_started_at ? new Date(job.generating_thumbnails_started_at) : job.phase2_started_at ? new Date(job.phase2_started_at) : null)?.toLocaleString()}
                      </p>
                      {(job.generating_thumbnails_started_at || job.finished_at) && (
                        <p className="text-xs text-primary/80 font-medium mt-0.5">
                          Duration: {formatDuration(
                            job.generating_thumbnails_started_at ?? job.phase2_started_at!,
                            job.finished_at ?? null
                          )}
                          {job.total_files != null && job.total_files > 0 && (
                            <span className="text-muted-foreground font-normal ml-1">
                              · {job.processed_files ?? job.total_files} thumbnails
                            </span>
                          )}
                        </p>
                      )}
                      {!job.finished_at && isThumbnailPhase && (
                        <span className="text-xs text-muted-foreground">in progress</span>
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
                      <span className="text-sm font-medium text-foreground">Started</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString()}</p>
                    </div>
                  </div>
                )}

                {/* Pending — not started yet */}
                {!job.started_at && (
                  <div className="flex items-start gap-4">
                    <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-warning shrink-0 z-10" />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">Waiting to start…</span>
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
                        {isCompleted ? "Completed" : isFailed ? "Failed" : "Cancelled"}
                      </span>
                      <p className="text-xs text-muted-foreground">{new Date(job.finished_at).toLocaleString()}</p>
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
                      label={isThumbnailOnly || isPhase2 ? "Generated" : "Processed"}
                      variant="primary"
                    />
                    <StatBox value={job.total_files} label="Total" />
                    <StatBox
                      value={Math.max(0, job.total_files - (job.processed_files ?? 0))}
                      label="Remaining"
                      variant={isCompleted ? "default" : "warning"}
                    />
                  </div>
                </>
              )}
              {job.current_file && (
                <div className="mt-4 p-3 bg-muted/50 rounded-lg">
                  <span className="text-xs text-muted-foreground uppercase tracking-wide">Current file</span>
                  <code className="block mt-1 text-xs font-mono text-foreground break-all">{job.current_file}</code>
                </div>
              )}
            </CardContent>
          </Card>
        )}

        {/* Index Statistics — index jobs only */}
        {job.stats_json && !isThumbnailOnly && (
          <Card>
            <CardHeader>
              <CardTitle>Index statistics</CardTitle>
              {job.started_at && (
                <CardDescription>
                  {formatDuration(job.started_at, job.finished_at)}
                  {speedCount > 0 && ` · ${formatSpeed(speedCount, durationMs)} scan rate`}
                </CardDescription>
              )}
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 sm:grid-cols-5 gap-4">
                <StatBox value={job.stats_json.scanned_files} label="Scanned" variant="success" />
                <StatBox value={job.stats_json.indexed_files} label="Indexed" variant="primary" />
                <StatBox value={job.stats_json.removed_files} label="Removed" variant="warning" />
                <StatBox value={job.stats_json.warnings ?? 0} label="Warnings" variant={(job.stats_json.warnings ?? 0) > 0 ? "warning" : "default"} />
                <StatBox value={job.stats_json.errors} label="Errors" variant={job.stats_json.errors > 0 ? "error" : "default"} />
              </div>
            </CardContent>
          </Card>
        )}

        {/* Thumbnail statistics — thumbnail-only jobs, completed */}
        {isThumbnailOnly && isCompleted && job.total_files != null && (
          <Card>
            <CardHeader>
              <CardTitle>Thumbnail statistics</CardTitle>
              {job.started_at && (
                <CardDescription>
                  {formatDuration(job.started_at, job.finished_at)}
                  {speedCount > 0 && ` · ${formatSpeed(speedCount, durationMs)} thumbnails/s`}
                </CardDescription>
              )}
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-4">
                <StatBox value={job.processed_files ?? job.total_files} label="Generated" variant="success" />
                <StatBox value={job.total_files} label="Total" />
              </div>
            </CardContent>
          </Card>
        )}

        {/* File errors */}
        {errors.length > 0 && (
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>File errors ({errors.length})</CardTitle>
              <CardDescription>Errors encountered while processing individual files</CardDescription>
            </CardHeader>
            <CardContent className="space-y-2 max-h-80 overflow-y-auto">
              {errors.map((error) => (
                <div key={error.id} className="p-3 bg-destructive/10 rounded-lg border border-destructive/20">
                  <code className="block text-sm font-mono text-destructive mb-1">{error.file_path}</code>
                  <p className="text-sm text-destructive/80">{error.error_message}</p>
                  <span className="text-xs text-muted-foreground">{new Date(error.created_at).toLocaleString()}</span>
                </div>
              ))}
            </CardContent>
          </Card>
        )}
      </div>
    </>
  );
}

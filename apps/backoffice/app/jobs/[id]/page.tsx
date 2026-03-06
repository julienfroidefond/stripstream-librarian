import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch } from "../../../lib/api";
import { Card, CardHeader, StatusBadge, JobTypeBadge, StatBox, ProgressBar } from "../../components/ui";

interface JobDetailPageProps {
  params: Promise<{ id: string }>;
}

interface JobDetails {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
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

function formatSpeed(stats: { scanned_files: number } | null, duration: number): string {
  if (!stats || duration === 0) return "-";
  const filesPerSecond = stats.scanned_files / (duration / 1000);
  return `${filesPerSecond.toFixed(1)} f/s`;
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

  const duration = job.started_at 
    ? new Date(job.finished_at || new Date()).getTime() - new Date(job.started_at).getTime()
    : 0;

  return (
    <>
      <div className="mb-6">
        <Link href="/jobs" className="inline-flex items-center text-sm text-muted hover:text-primary transition-colors">
          ← Back to jobs
        </Link>
        <h1 className="text-3xl font-bold text-foreground mt-2">Job Details</h1>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Overview Card */}
        <Card>
          <CardHeader title="Overview" />
          <div className="space-y-3">
            <div className="flex items-center justify-between py-2 border-b border-line">
              <span className="text-sm text-muted">ID</span>
              <code className="px-2 py-1 bg-muted/10 rounded font-mono text-sm text-foreground">{job.id}</code>
            </div>
            <div className="flex items-center justify-between py-2 border-b border-line">
              <span className="text-sm text-muted">Type</span>
              <JobTypeBadge type={job.type} />
            </div>
            <div className="flex items-center justify-between py-2 border-b border-line">
              <span className="text-sm text-muted">Status</span>
              <StatusBadge status={job.status} />
            </div>
            <div className="flex items-center justify-between py-2">
              <span className="text-sm text-muted">Library</span>
              <span className="text-sm text-foreground">{job.library_id || "All libraries"}</span>
            </div>
          </div>
        </Card>

        {/* Timeline Card */}
        <Card>
          <CardHeader title="Timeline" />
          <div className="space-y-4">
            <div className="flex items-start gap-4">
              <div className={`w-2 h-2 rounded-full mt-2 ${job.created_at ? 'bg-success' : 'bg-muted'}`} />
              <div className="flex-1">
                <span className="text-sm font-medium text-foreground">Created</span>
                <p className="text-sm text-muted">{new Date(job.created_at).toLocaleString()}</p>
              </div>
            </div>
            <div className="flex items-start gap-4">
              <div className={`w-2 h-2 rounded-full mt-2 ${job.started_at ? 'bg-success' : job.created_at ? 'bg-warning' : 'bg-muted'}`} />
              <div className="flex-1">
                <span className="text-sm font-medium text-foreground">Started</span>
                <p className="text-sm text-muted">
                  {job.started_at ? new Date(job.started_at).toLocaleString() : "Pending..."}
                </p>
              </div>
            </div>
            <div className="flex items-start gap-4">
              <div className={`w-2 h-2 rounded-full mt-2 ${job.finished_at ? 'bg-success' : job.started_at ? 'bg-primary animate-pulse' : 'bg-muted'}`} />
              <div className="flex-1">
                <span className="text-sm font-medium text-foreground">Finished</span>
                <p className="text-sm text-muted">
                  {job.finished_at 
                    ? new Date(job.finished_at).toLocaleString()
                    : job.started_at 
                      ? "Running..."
                      : "Waiting..."
                  }
                </p>
              </div>
            </div>
          </div>
          {job.started_at && (
            <div className="mt-4 inline-flex items-center px-3 py-1.5 bg-primary-soft text-primary rounded-lg text-sm font-medium">
              Duration: {formatDuration(job.started_at, job.finished_at)}
            </div>
          )}
        </Card>

        {/* Progress Card */}
        {(job.status === "running" || job.status === "success" || job.status === "failed") && (
          <Card>
            <CardHeader title="Progress" />
            {job.total_files && job.total_files > 0 && (
              <>
                <ProgressBar value={job.progress_percent || 0} showLabel size="lg" className="mb-4" />
                <div className="grid grid-cols-3 gap-4">
                  <StatBox value={job.processed_files || 0} label="Processed" variant="primary" />
                  <StatBox value={job.total_files} label="Total" />
                  <StatBox value={job.total_files - (job.processed_files || 0)} label="Remaining" variant="warning" />
                </div>
              </>
            )}
            {job.current_file && (
              <div className="mt-4 p-3 bg-muted/5 rounded-lg">
                <span className="text-sm text-muted">Current file:</span>
                <code className="block mt-1 text-xs font-mono text-foreground truncate">{job.current_file}</code>
              </div>
            )}
          </Card>
        )}

        {/* Statistics Card */}
        {job.stats_json && (
          <Card>
            <CardHeader title="Statistics" />
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-4">
              <StatBox value={job.stats_json.scanned_files} label="Scanned" variant="success" />
              <StatBox value={job.stats_json.indexed_files} label="Indexed" variant="primary" />
              <StatBox value={job.stats_json.removed_files} label="Removed" variant="warning" />
              <StatBox value={job.stats_json.errors} label="Errors" variant={job.stats_json.errors > 0 ? "error" : "default"} />
            </div>
            {job.started_at && (
              <div className="flex items-center justify-between py-2 border-t border-line">
                <span className="text-sm text-muted">Speed:</span>
                <span className="text-sm font-medium text-foreground">{formatSpeed(job.stats_json, duration)}</span>
              </div>
            )}
          </Card>
        )}

        {/* Errors Card */}
        {errors.length > 0 && (
          <Card className="lg:col-span-2">
            <CardHeader title={`Errors (${errors.length})`} />
            <div className="space-y-2 max-h-80 overflow-y-auto">
              {errors.map((error) => (
                <div key={error.id} className="p-3 bg-error-soft rounded-lg">
                  <code className="block text-sm font-mono text-error mb-1">{error.file_path}</code>
                  <p className="text-sm text-error/80">{error.error_message}</p>
                  <span className="text-xs text-muted">{new Date(error.created_at).toLocaleString()}</span>
                </div>
              ))}
            </div>
          </Card>
        )}

        {/* Error Message */}
        {job.error_opt && (
          <Card className="lg:col-span-2">
            <CardHeader title="Error" />
            <pre className="p-4 bg-error-soft rounded-lg text-sm text-error overflow-x-auto">{job.error_opt}</pre>
          </Card>
        )}
      </div>
    </>
  );
}

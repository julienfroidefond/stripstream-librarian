import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch } from "../../../lib/api";

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
      <div className="page-header">
        <Link href="/jobs" className="back-link">← Back to jobs</Link>
        <h1>Job Details</h1>
      </div>

      <div className="job-detail-grid">
        {/* Overview Card */}
        <div className="card job-overview">
          <h2>Overview</h2>
          <div className="job-meta">
            <div className="meta-item">
              <span className="meta-label">ID</span>
              <code className="meta-value">{job.id}</code>
            </div>
            <div className="meta-item">
              <span className="meta-label">Type</span>
              <span className={`meta-value job-type ${job.type}`}>{job.type}</span>
            </div>
            <div className="meta-item">
              <span className="meta-label">Status</span>
              <span className={`meta-value status-badge status-${job.status}`}>{job.status}</span>
            </div>
            <div className="meta-item">
              <span className="meta-label">Library</span>
              <span className="meta-value">{job.library_id || "All libraries"}</span>
            </div>
          </div>
        </div>

        {/* Timeline Card */}
        <div className="card job-timeline">
          <h2>Timeline</h2>
          <div className="timeline">
            <div className={`timeline-item ${job.created_at ? 'completed' : ''}`}>
              <div className="timeline-dot" />
              <div className="timeline-content">
                <span className="timeline-label">Created</span>
                <span className="timeline-time">{new Date(job.created_at).toLocaleString()}</span>
              </div>
            </div>
            <div className={`timeline-item ${job.started_at ? 'completed' : ''} ${!job.started_at ? 'pending' : ''}`}>
              <div className="timeline-dot" />
              <div className="timeline-content">
                <span className="timeline-label">Started</span>
                <span className="timeline-time">
                  {job.started_at ? new Date(job.started_at).toLocaleString() : "Pending..."}
                </span>
              </div>
            </div>
            <div className={`timeline-item ${job.finished_at ? 'completed' : ''} ${job.started_at && !job.finished_at ? 'active' : ''} ${!job.started_at ? 'pending' : ''}`}>
              <div className="timeline-dot" />
              <div className="timeline-content">
                <span className="timeline-label">Finished</span>
                <span className="timeline-time">
                  {job.finished_at 
                    ? new Date(job.finished_at).toLocaleString()
                    : job.started_at 
                      ? "Running..."
                      : "Waiting..."
                  }
                </span>
              </div>
            </div>
          </div>
          {job.started_at && (
            <div className="duration-badge">
              Duration: {formatDuration(job.started_at, job.finished_at)}
            </div>
          )}
        </div>

        {/* Progress Card */}
        {(job.status === "running" || job.status === "success" || job.status === "failed") && (
          <div className="card job-progress-detail">
            <h2>Progress</h2>
            {job.total_files && job.total_files > 0 && (
              <>
                <div className="progress-bar-large">
                  <div 
                    className="progress-fill" 
                    style={{ width: `${job.progress_percent || 0}%` }}
                  />
                  <span className="progress-text">{job.progress_percent || 0}%</span>
                </div>
                <div className="progress-stats-grid">
                  <div className="stat-box">
                    <span className="stat-value">{job.processed_files || 0}</span>
                    <span className="stat-label">Processed</span>
                  </div>
                  <div className="stat-box">
                    <span className="stat-value">{job.total_files}</span>
                    <span className="stat-label">Total</span>
                  </div>
                  <div className="stat-box">
                    <span className="stat-value">{job.total_files - (job.processed_files || 0)}</span>
                    <span className="stat-label">Remaining</span>
                  </div>
                </div>
              </>
            )}
            {job.current_file && (
              <div className="current-file-box">
                <span className="label">Current file:</span>
                <code className="file-path">{job.current_file}</code>
              </div>
            )}
          </div>
        )}

        {/* Statistics Card */}
        {job.stats_json && (
          <div className="card job-statistics">
            <h2>Statistics</h2>
            <div className="stats-grid">
              <div className="stat-item">
                <span className="stat-number success">{job.stats_json.scanned_files}</span>
                <span className="stat-label">Scanned</span>
              </div>
              <div className="stat-item">
                <span className="stat-number primary">{job.stats_json.indexed_files}</span>
                <span className="stat-label">Indexed</span>
              </div>
              <div className="stat-item">
                <span className="stat-number warning">{job.stats_json.removed_files}</span>
                <span className="stat-label">Removed</span>
              </div>
              <div className="stat-item">
                <span className={`stat-number ${job.stats_json.errors > 0 ? 'error' : ''}`}>
                  {job.stats_json.errors}
                </span>
                <span className="stat-label">Errors</span>
              </div>
            </div>
            {job.started_at && (
              <div className="speed-stat">
                <span className="speed-label">Speed:</span>
                <span className="speed-value">{formatSpeed(job.stats_json, duration)}</span>
              </div>
            )}
          </div>
        )}

        {/* Errors Card */}
        {errors.length > 0 && (
          <div className="card job-errors">
            <h2>Errors ({errors.length})</h2>
            <div className="errors-list">
              {errors.map((error) => (
                <div key={error.id} className="error-item">
                  <code className="error-file">{error.file_path}</code>
                  <span className="error-message">{error.error_message}</span>
                  <span className="error-time">{new Date(error.created_at).toLocaleString()}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Error Message */}
        {job.error_opt && (
          <div className="card job-error-message">
            <h2>Error</h2>
            <pre className="error-details">{job.error_opt}</pre>
          </div>
        )}
      </div>
    </>
  );
}

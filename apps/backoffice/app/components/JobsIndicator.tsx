"use client";

import { useEffect, useState } from "react";
import Link from "next/link";

interface Job {
  id: string;
  status: string;
  current_file: string | null;
  progress_percent: number | null;
}

interface JobsIndicatorProps {
  apiBaseUrl: string;
  apiToken: string;
}

export function JobsIndicator({ apiBaseUrl, apiToken }: JobsIndicatorProps) {
  const [activeJobs, setActiveJobs] = useState<Job[]>([]);
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const fetchActiveJobs = async () => {
      try {
        const response = await fetch(`${apiBaseUrl}/index/jobs/active`, {
          headers: {
            "Authorization": `Bearer ${apiToken}`,
          },
        });

        if (response.ok) {
          const jobs = await response.json();
          // Enrich with details for running jobs
          const jobsWithDetails = await Promise.all(
            jobs.map(async (job: Job) => {
              if (job.status === "running") {
                try {
                  const detailRes = await fetch(`${apiBaseUrl}/index/jobs/${job.id}`, {
                    headers: { "Authorization": `Bearer ${apiToken}` },
                  });
                  if (detailRes.ok) {
                    const detail = await detailRes.json();
                    return { ...job, ...detail };
                  }
                } catch {
                  // ignore detail fetch errors
                }
              }
              return job;
            })
          );
          setActiveJobs(jobsWithDetails);
        }
      } catch {
        // Silently fail
      }
    };

    fetchActiveJobs();
    const interval = setInterval(fetchActiveJobs, 5000);

    return () => clearInterval(interval);
  }, [apiBaseUrl, apiToken]);

  const pendingCount = activeJobs.filter(j => j.status === "pending").length;
  const runningCount = activeJobs.filter(j => j.status === "running").length;
  const totalCount = activeJobs.length;

  if (totalCount === 0) {
    return (
      <Link href="/jobs" className="jobs-indicator empty">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <path d="M8 12h8M12 8v8" />
        </svg>
      </Link>
    );
  }

  return (
    <div className="jobs-indicator-container">
      <button 
        className="jobs-indicator active" 
        onClick={() => setIsOpen(!isOpen)}
        title={`${totalCount} active job${totalCount !== 1 ? 's' : ''}`}
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <path d="M8 12h8M12 8v8" />
        </svg>
        {totalCount > 0 && (
          <span className="jobs-badge">
            {totalCount > 9 ? "9+" : totalCount}
          </span>
        )}
        {runningCount > 0 && (
          <span className="jobs-pulse" />
        )}
      </button>

      {isOpen && (
        <div className="jobs-dropdown">
          <div className="jobs-dropdown-header">
            <strong>Active Jobs</strong>
            <Link href="/jobs" onClick={() => setIsOpen(false)}>View all</Link>
          </div>
          
          {activeJobs.length === 0 ? (
            <p className="jobs-empty">No active jobs</p>
          ) : (
            <ul className="jobs-list">
              {activeJobs.map(job => (
                <li key={job.id} className={`job-item job-${job.status}`}>
                  <div className="job-header">
                    <span className={`job-status status-${job.status}`}>
                      {job.status}
                    </span>
                    <code className="job-id">{job.id.slice(0, 8)}</code>
                  </div>
                  {job.status === "running" && job.progress_percent !== null && (
                    <div className="job-mini-progress">
                      <div 
                        className="job-progress-bar" 
                        style={{ width: `${job.progress_percent}%` }}
                      />
                      <span>{job.progress_percent}%</span>
                    </div>
                  )}
                  {job.current_file && (
                    <p className="job-file" title={job.current_file}>
                      {job.current_file.length > 30 
                        ? job.current_file.substring(0, 30) + "..." 
                        : job.current_file}
                    </p>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

"use client";

import { useState } from "react";
import Link from "next/link";
import { JobProgress } from "./JobProgress";

interface JobRowProps {
  job: {
    id: string;
    library_id: string | null;
    type: string;
    status: string;
    created_at: string;
    error_opt: string | null;
  };
  libraryName: string | undefined;
  highlighted?: boolean;
  onCancel: (id: string) => void;
}

export function JobRow({ job, libraryName, highlighted, onCancel }: JobRowProps) {
  const [showProgress, setShowProgress] = useState(
    highlighted || job.status === "running" || job.status === "pending"
  );

  const handleComplete = () => {
    setShowProgress(false);
    // Trigger a page refresh to update the job status
    window.location.reload();
  };

  return (
    <>
      <tr className={highlighted ? "job-highlighted" : undefined}>
        <td>
          <Link href={`/jobs/${job.id}`} className="job-id-link">
            <code>{job.id.slice(0, 8)}</code>
          </Link>
        </td>
        <td>{job.library_id ? libraryName || job.library_id.slice(0, 8) : "—"}</td>
        <td>{job.type}</td>
        <td>
          <span className={`status-${job.status}`}>{job.status}</span>
          {job.error_opt && <span className="error-hint" title={job.error_opt}>!</span>}
          {(job.status === "running" || job.status === "pending") && (
            <button 
              className="toggle-progress-btn"
              onClick={() => setShowProgress(!showProgress)}
            >
              {showProgress ? "Hide" : "Show"} progress
            </button>
          )}
        </td>
        <td>{new Date(job.created_at).toLocaleString()}</td>
        <td>
          <div style={{ display: "flex", gap: "8px" }}>
            <Link href={`/jobs/${job.id}`} className="view-btn">
              View
            </Link>
            {(job.status === "pending" || job.status === "running") && (
              <button 
                className="cancel-btn"
                onClick={() => onCancel(job.id)}
              >
                Cancel
              </button>
            )}
          </div>
        </td>
      </tr>
      {showProgress && (job.status === "running" || job.status === "pending") && (
        <tr className="progress-row">
          <td colSpan={6}>
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

"use client";

import { useState } from "react";
import Link from "next/link";
import { JobProgress } from "./JobProgress";
import { StatusBadge, Button } from "./ui";

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
    window.location.reload();
  };

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
        <td className="px-4 py-3 text-sm text-muted">
          {new Date(job.created_at).toLocaleString()}
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
          <td colSpan={6} className="px-4 py-3 bg-muted/5">
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

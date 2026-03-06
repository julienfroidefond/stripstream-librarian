"use client";

import { useState, useEffect } from "react";
import { JobRow } from "./JobRow";

interface Job {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  created_at: string;
  error_opt: string | null;
}

interface JobsListProps {
  initialJobs: Job[];
  libraries: Map<string, string>;
  highlightJobId?: string;
}

export function JobsList({ initialJobs, libraries, highlightJobId }: JobsListProps) {
  const [jobs, setJobs] = useState(initialJobs);

  // Refresh jobs list via SSE
  useEffect(() => {
    const eventSource = new EventSource("/api/jobs/stream");
    
    eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        if (Array.isArray(data)) {
          setJobs(data);
        }
      } catch (error) {
        console.error("Failed to parse SSE data:", error);
      }
    };
    
    eventSource.onerror = (err) => {
      console.error("SSE error:", err);
      eventSource.close();
    };

    return () => {
      eventSource.close();
    };
  }, []);

  const handleCancel = async (id: string) => {
    try {
      const response = await fetch(`/api/jobs/${id}/cancel`, {
        method: "POST",
      });

      if (response.ok) {
        // Update local state to reflect cancellation
        setJobs(jobs.map(job => 
          job.id === id ? { ...job, status: "cancelled" } : job
        ));
      }
    } catch (error) {
      console.error("Failed to cancel job:", error);
    }
  };

  return (
    <div className="bg-card rounded-xl shadow-soft border border-line overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-line bg-muted/5">
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">ID</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Library</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Type</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Status</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Created</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {jobs.map((job) => (
              <JobRow
                key={job.id}
                job={job}
                libraryName={job.library_id ? libraries.get(job.library_id) : undefined}
                highlighted={job.id === highlightJobId}
                onCancel={handleCancel}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

"use client";

import { useState, useEffect } from "react";
import { useTranslation } from "../../lib/i18n/context";
import { JobRow } from "./JobRow";

interface Job {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  error_opt: string | null;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
  } | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
}

interface JobsListProps {
  initialJobs: Job[];
  libraries: Map<string, string>;
  highlightJobId?: string;
}

function formatDuration(start: string, end: string | null): string {
  const startDate = new Date(start);
  const endDate = end ? new Date(end) : new Date();
  const diff = endDate.getTime() - startDate.getTime();
  
  if (diff < 60000) return `${Math.floor(diff / 1000)}s`;
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ${Math.floor((diff % 60000) / 1000)}s`;
  return `${Math.floor(diff / 3600000)}h ${Math.floor((diff % 3600000) / 60000)}m`;
}

function getDateParts(dateStr: string): { mins: number; hours: number; useDate: boolean; date: Date } {
  const date = new Date(dateStr);
  const now = new Date();
  const diff = now.getTime() - date.getTime();

  if (diff < 3600000) {
    const mins = Math.floor(diff / 60000);
    return { mins, hours: 0, useDate: false, date };
  }
  if (diff < 86400000) {
    const hours = Math.floor(diff / 3600000);
    return { mins: 0, hours, useDate: false, date };
  }
  return { mins: 0, hours: 0, useDate: true, date };
}

export function JobsList({ initialJobs, libraries, highlightJobId }: JobsListProps) {
  const { t } = useTranslation();
  const [jobs, setJobs] = useState(initialJobs);

  const formatDate = (dateStr: string): string => {
    const parts = getDateParts(dateStr);
    if (parts.useDate) {
      return parts.date.toLocaleDateString();
    }
    if (parts.mins < 1) return t("time.justNow");
    if (parts.hours > 0) return t("time.hoursAgo", { count: parts.hours });
    return t("time.minutesAgo", { count: parts.mins });
  };

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
    const response = await fetch(`/api/jobs/${id}/cancel`, { method: "POST" });
    if (response.ok) {
      setJobs(jobs.map(job =>
        job.id === id ? { ...job, status: "cancelled" } : job
      ));
    } else {
      const data = await response.json().catch(() => ({}));
      console.error("Failed to cancel job:", data?.error ?? response.status);
    }
  };

  return (
    <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-border/60 bg-muted/50">
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.id")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.library")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.type")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.status")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.files")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.thumbnails")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.duration")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.created")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.actions")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border/60">
            {jobs.map((job) => (
              <JobRow
                key={job.id}
                job={job}
                libraryName={job.library_id ? libraries.get(job.library_id) : undefined}
                highlighted={job.id === highlightJobId}
                onCancel={handleCancel}
                formatDate={formatDate}
                formatDuration={formatDuration}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

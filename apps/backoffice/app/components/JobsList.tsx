"use client";

import { useState, useEffect } from "react";
import { useTranslation } from "../../lib/i18n/context";
import { JobRow } from "./JobRow";

const PAGE_SIZE = 25;

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
    refreshed?: number;
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

export function JobsList({ initialJobs, libraries, highlightJobId }: JobsListProps) {
  const { t, locale } = useTranslation();
  const [jobs, setJobs] = useState(initialJobs);
  const [filterType, setFilterType] = useState<string>("");
  const [filterStatus, setFilterStatus] = useState<string>("");
  const [filterLibrary, setFilterLibrary] = useState<string>("");

  const filteredJobs = jobs.filter((job) => {
    if (filterType && job.type !== filterType) return false;
    if (filterStatus && job.status !== filterStatus) return false;
    if (filterLibrary && job.library_id !== filterLibrary) return false;
    return true;
  });

  const initialPage = highlightJobId
    ? Math.ceil((filteredJobs.findIndex(j => j.id === highlightJobId) + 1) / PAGE_SIZE) || 1
    : 1;
  const [currentPage, setCurrentPage] = useState(initialPage);

  const formatDate = (dateStr: string): string => {
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return dateStr;
    const loc = locale === "fr" ? "fr-FR" : "en-US";
    return date.toLocaleString(loc, {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  // Derive unique types, statuses, libraries for filter options
  const jobTypes = [...new Set(jobs.map(j => j.type))].sort();
  const jobStatuses = [...new Set(jobs.map(j => j.status))].sort();
  const jobLibraryIds = [...new Set(jobs.map(j => j.library_id).filter(Boolean))] as string[];

  const totalPages = Math.ceil(filteredJobs.length / PAGE_SIZE);
  const pageStart = (currentPage - 1) * PAGE_SIZE;
  const visibleJobs = filteredJobs.slice(pageStart, pageStart + PAGE_SIZE);

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

  const refreshJobs = async () => {
    try {
      const res = await fetch("/api/jobs/list");
      if (res.ok) {
        const data = await res.json();
        if (Array.isArray(data)) setJobs(data);
      }
    } catch { /* SSE will catch up */ }
  };

  const handleReplay = async (id: string) => {
    const response = await fetch(`/api/jobs/${id}/replay`, { method: "POST" });
    if (response.ok) {
      await refreshJobs();
    } else {
      const data = await response.json().catch(() => ({}));
      console.error("Failed to replay job:", data?.error ?? response.status);
    }
  };

  return (
    <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden">
      <div className="flex items-center gap-3 px-4 py-3 border-b border-border/60 bg-muted/30 flex-wrap">
        <select
          value={filterType}
          onChange={(e) => { setFilterType(e.target.value); setCurrentPage(1); }}
          className="text-xs border border-border rounded-md px-2.5 py-1.5 bg-background focus:outline-none focus:ring-2 focus:ring-ring"
        >
          <option value="">{t("jobsList.allTypes")}</option>
          {jobTypes.map((type) => (
            <option key={type} value={type}>{t(`jobType.${type}` as never) || type}</option>
          ))}
        </select>
        <select
          value={filterStatus}
          onChange={(e) => { setFilterStatus(e.target.value); setCurrentPage(1); }}
          className="text-xs border border-border rounded-md px-2.5 py-1.5 bg-background focus:outline-none focus:ring-2 focus:ring-ring"
        >
          <option value="">{t("jobsList.allStatuses")}</option>
          {jobStatuses.map((status) => (
            <option key={status} value={status}>{status}</option>
          ))}
        </select>
        <select
          value={filterLibrary}
          onChange={(e) => { setFilterLibrary(e.target.value); setCurrentPage(1); }}
          className="text-xs border border-border rounded-md px-2.5 py-1.5 bg-background focus:outline-none focus:ring-2 focus:ring-ring"
        >
          <option value="">{t("jobsList.allLibraries")}</option>
          {jobLibraryIds.map((id) => (
            <option key={id} value={id}>{libraries.get(id) || id}</option>
          ))}
        </select>
        {(filterType || filterStatus || filterLibrary) && (
          <button
            onClick={() => { setFilterType(""); setFilterStatus(""); setFilterLibrary(""); setCurrentPage(1); }}
            className="text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            {t("common.clear")}
          </button>
        )}
        <span className="text-xs text-muted-foreground ml-auto">
          {filteredJobs.length} / {jobs.length}
        </span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-border/60 bg-muted/50">
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.id")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.library")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.type")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.status")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.stats")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.duration")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.created")}</th>
              <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobsList.actions")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border/60">
            {visibleJobs.map((job) => (
              <JobRow
                key={job.id}
                job={job}
                libraryName={job.library_id ? libraries.get(job.library_id) : undefined}
                highlighted={job.id === highlightJobId}
                onCancel={handleCancel}
                onReplay={handleReplay}
                formatDate={formatDate}
                formatDuration={formatDuration}
              />
            ))}
          </tbody>
        </table>
      </div>
      {totalPages > 1 && (
        <div className="flex items-center justify-between px-4 py-3 border-t border-border/60">
          <span className="text-xs text-muted-foreground">
            {t("pagination.range", {
              start: pageStart + 1,
              end: Math.min(pageStart + PAGE_SIZE, filteredJobs.length),
              total: filteredJobs.length,
            })}
          </span>
          <div className="flex items-center gap-1">
            <button
              onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
              disabled={currentPage === 1}
              className="px-2.5 py-1.5 text-xs rounded-md border border-input bg-background hover:bg-accent/50 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {t("pagination.previous")}
            </button>
            {Array.from({ length: totalPages }, (_, i) => i + 1)
              .filter(p => p === 1 || p === totalPages || Math.abs(p - currentPage) <= 1)
              .reduce<(number | "…")[]>((acc, p, i, arr) => {
                if (i > 0 && p - (arr[i - 1] as number) > 1) acc.push("…");
                acc.push(p);
                return acc;
              }, [])
              .map((p, i) =>
                p === "…" ? (
                  <span key={`ellipsis-${i}`} className="px-1.5 text-xs text-muted-foreground">…</span>
                ) : (
                  <button
                    key={p}
                    onClick={() => setCurrentPage(p as number)}
                    className={`min-w-[2rem] px-2.5 py-1.5 text-xs rounded-md border transition-colors ${
                      currentPage === p
                        ? "border-primary bg-primary text-white"
                        : "border-input bg-background hover:bg-accent/50"
                    }`}
                  >
                    {p}
                  </button>
                )
              )}
            <button
              onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
              disabled={currentPage === totalPages}
              className="px-2.5 py-1.5 text-xs rounded-md border border-input bg-background hover:bg-accent/50 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {t("pagination.next")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

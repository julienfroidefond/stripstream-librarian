"use client";

import { useEffect, useState, useRef } from "react";
import Link from "next/link";

interface Job {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  current_file: string | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    errors: number;
  } | null;
}

export function JobsIndicator() {
  const [activeJobs, setActiveJobs] = useState<Job[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const fetchActiveJobs = async () => {
      try {
        const response = await fetch("/api/jobs/active");
        if (response.ok) {
          const jobs = await response.json();
          setActiveJobs(jobs);
        }
      } catch (error) {
        console.error("Failed to fetch jobs:", error);
      }
    };

    fetchActiveJobs();
    const interval = setInterval(fetchActiveJobs, 2000);
    return () => clearInterval(interval);
  }, []);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const runningJobs = activeJobs.filter(j => j.status === "running");
  const pendingJobs = activeJobs.filter(j => j.status === "pending");
  const totalCount = activeJobs.length;

  // Calculate overall progress
  const totalProgress = runningJobs.reduce((acc, job) => {
    return acc + (job.progress_percent || 0);
  }, 0) / (runningJobs.length || 1);

  if (totalCount === 0) {
    return (
      <Link 
        href="/jobs" 
        className="flex items-center justify-center w-10 h-10 rounded-lg text-muted transition-all duration-200 hover:text-foreground hover:bg-primary-soft"
        title="View all jobs"
      >
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="2" y="3" width="20" height="18" rx="2" />
          <path d="M6 8h12M6 12h12M6 16h8" />
        </svg>
      </Link>
    );
  }

  return (
    <div className="relative" ref={dropdownRef}>
      <button 
        className={`flex items-center gap-2 px-3 py-2 rounded-lg font-medium text-sm transition-all duration-200 ${
          runningJobs.length > 0 
            ? 'bg-success-soft text-success' 
            : 'bg-warning-soft text-warning'
        } ${isOpen ? 'ring-2 ring-primary' : ''}`}
        onClick={() => setIsOpen(!isOpen)}
        title={`${totalCount} active job${totalCount !== 1 ? 's' : ''}`}
      >
        {/* Animated spinner for running jobs */}
        {runningJobs.length > 0 && (
          <div className="w-4 h-4 animate-spin">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
              <path d="M12 2a10 10 0 0 1 10 10" strokeLinecap="round" />
            </svg>
          </div>
        )}
        
        {/* Icon */}
        <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="2" y="3" width="20" height="18" rx="2" />
          <path d="M6 8h12M6 12h12M6 16h8" />
        </svg>
        
        {/* Badge with count */}
        <span className="flex items-center justify-center min-w-5 h-5 px-1.5 text-xs font-bold text-white bg-current rounded-full">
          <span className="text-background">{totalCount > 99 ? "99+" : totalCount}</span>
        </span>
        
        {/* Chevron */}
        <svg 
          className={`w-4 h-4 transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`}
          viewBox="0 0 24 24" 
          fill="none" 
          stroke="currentColor" 
          strokeWidth="2"
        >
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>

      {/* Popin/Dropdown */}
      {isOpen && (
        <div className="absolute right-0 top-full mt-2 w-96 bg-card rounded-xl shadow-card border border-line overflow-hidden z-50">
          <div className="flex items-center justify-between px-4 py-3 border-b border-line bg-muted/5">
            <div className="flex items-center gap-3">
              <span className="text-2xl">📊</span>
              <div>
                <h3 className="font-semibold text-foreground">Active Jobs</h3>
                <p className="text-xs text-muted">
                  {runningJobs.length > 0 
                    ? `${runningJobs.length} running, ${pendingJobs.length} pending`
                    : `${pendingJobs.length} job${pendingJobs.length !== 1 ? 's' : ''} pending`
                  }
                </p>
              </div>
            </div>
            <Link 
              href="/jobs" 
              className="text-sm font-medium text-primary hover:text-primary/80 transition-colors"
              onClick={() => setIsOpen(false)}
            >
              View All →
            </Link>
          </div>

          {/* Overall progress bar if running */}
          {runningJobs.length > 0 && (
            <div className="px-4 py-3 border-b border-line">
              <div className="flex items-center justify-between text-sm mb-2">
                <span className="text-muted">Overall Progress</span>
                <span className="font-semibold text-foreground">{Math.round(totalProgress)}%</span>
              </div>
              <div className="h-2 bg-line rounded-full overflow-hidden">
                <div 
                  className="h-full bg-success rounded-full transition-all duration-500"
                  style={{ width: `${totalProgress}%` }}
                />
              </div>
            </div>
          )}
          
          <div className="max-h-80 overflow-y-auto">
            {activeJobs.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-8 text-muted">
                <span className="text-4xl mb-2">✅</span>
                <p>No active jobs</p>
              </div>
            ) : (
              <ul className="divide-y divide-line">
                {activeJobs.map(job => (
                  <li key={job.id}>
                    <Link 
                      href={`/jobs/${job.id}`}
                      className="block px-4 py-3 hover:bg-muted/5 transition-colors"
                      onClick={() => setIsOpen(false)}
                    >
                      <div className="flex items-start gap-3">
                        <div className="mt-0.5">
                          {job.status === "running" && <span className="animate-spin inline-block">⏳</span>}
                          {job.status === "pending" && <span>⏸</span>}
                        </div>
                        
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <code className="text-xs px-1.5 py-0.5 bg-line/50 rounded font-mono">{job.id.slice(0, 8)}</code>
                            <span className={`text-xs px-2 py-0.5 rounded font-medium ${
                              job.type === 'rebuild' ? 'bg-primary-soft text-primary' : 'bg-muted/20 text-muted'
                            }`}>
                              {job.type}
                            </span>
                          </div>
                          
                          {job.status === "running" && job.progress_percent !== null && (
                            <div className="flex items-center gap-2 mt-2">
                              <div className="flex-1 h-1.5 bg-line rounded-full overflow-hidden">
                                <div 
                                  className="h-full bg-success rounded-full transition-all duration-300"
                                  style={{ width: `${job.progress_percent}%` }}
                                />
                              </div>
                              <span className="text-xs font-medium text-muted">{job.progress_percent}%</span>
                            </div>
                          )}
                          
                          {job.current_file && (
                            <p className="text-xs text-muted mt-1.5 truncate" title={job.current_file}>
                              📄 {job.current_file}
                            </p>
                          )}
                          
                          {job.stats_json && (
                            <div className="flex items-center gap-3 mt-1.5 text-xs text-muted">
                              <span>✓ {job.stats_json.indexed_files}</span>
                              {job.stats_json.errors > 0 && (
                                <span className="text-error">⚠ {job.stats_json.errors}</span>
                              )}
                            </div>
                          )}
                        </div>
                      </div>
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* Footer */}
          <div className="px-4 py-2 border-t border-line bg-muted/5">
            <p className="text-xs text-muted text-center">Auto-refreshing every 2s</p>
          </div>
        </div>
      )}
    </div>
  );
}

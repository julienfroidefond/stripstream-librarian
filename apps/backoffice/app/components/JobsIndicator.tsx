"use client";

import { useEffect, useState, useRef } from "react";
import Link from "next/link";
import { Button } from "./ui/Button";
import { Badge } from "./ui/Badge";
import { ProgressBar } from "./ui/ProgressBar";

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

// Icons
const JobsIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
    <rect x="2" y="3" width="20" height="18" rx="2" />
    <path d="M6 8h12M6 12h12M6 16h8" strokeLinecap="round" />
  </svg>
);

const SpinnerIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
    <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
    <path d="M12 2a10 10 0 0 1 10 10" strokeLinecap="round" />
  </svg>
);

const ChevronIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M6 9l6 6 6-6" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

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

  const runningJobs = activeJobs.filter(j => j.status === "running" || j.status === "generating_thumbnails");
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
        className="
          flex items-center justify-center 
          w-9 h-9 
          rounded-md 
          text-muted-foreground 
          hover:text-foreground 
          hover:bg-accent 
          transition-colors duration-200
        "
        title="View all jobs"
      >
        <JobsIcon className="w-[18px] h-[18px]" />
      </Link>
    );
  }

  return (
    <div className="relative" ref={dropdownRef}>
      <button 
        className={`
          flex items-center gap-2 
          px-3 py-2 
          rounded-md 
          font-medium text-sm 
          transition-all duration-200
          ${runningJobs.length > 0 
            ? 'bg-success/10 text-success hover:bg-success/20' 
            : 'bg-warning/10 text-warning hover:bg-warning/20'
          }
          ${isOpen ? 'ring-2 ring-ring ring-offset-2 ring-offset-background' : ''}
        `}
        onClick={() => setIsOpen(!isOpen)}
        title={`${totalCount} active job${totalCount !== 1 ? 's' : ''}`}
      >
        {/* Animated spinner for running jobs */}
        {runningJobs.length > 0 && (
          <div className="w-4 h-4 animate-spin">
            <SpinnerIcon className="w-4 h-4" />
          </div>
        )}
        
        {/* Icon */}
        <JobsIcon className="w-4 h-4" />
        
        {/* Badge with count */}
        <span className="flex items-center justify-center min-w-5 h-5 px-1.5 text-xs font-bold bg-current rounded-full">
          <span className="text-background">{totalCount > 99 ? "99+" : totalCount}</span>
        </span>
        
        {/* Chevron */}
        <ChevronIcon 
          className={`w-4 h-4 transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`} 
        />
      </button>

      {/* Backdrop mobile */}
      {isOpen && (
        <div
          className="fixed inset-0 z-40 sm:hidden bg-background/60 backdrop-blur-sm"
          onClick={() => setIsOpen(false)}
          aria-hidden="true"
        />
      )}

      {/* Popin/Dropdown with glassmorphism */}
      {isOpen && (
        <div className="
          fixed sm:absolute
          inset-x-3 sm:inset-x-auto
          top-[4.5rem] sm:top-full sm:mt-2
          sm:w-96
          bg-popover/95 backdrop-blur-md
          rounded-xl
          shadow-elevation-2
          border border-border/60
          overflow-hidden
          z-50
          animate-scale-in
        ">
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-border/60 bg-muted/50">
            <div className="flex items-center gap-3">
              <span className="text-xl">📊</span>
              <div>
                <h3 className="font-semibold text-foreground">Active Jobs</h3>
                <p className="text-xs text-muted-foreground">
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
            <div className="px-4 py-3 border-b border-border/60">
              <div className="flex items-center justify-between text-sm mb-2">
                <span className="text-muted-foreground">Overall Progress</span>
                <span className="font-semibold text-foreground">{Math.round(totalProgress)}%</span>
              </div>
              <ProgressBar value={totalProgress} size="sm" variant="success" />
            </div>
          )}
          
          {/* Job List */}
          <div className="max-h-80 overflow-y-auto scrollbar-hide">
            {activeJobs.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
                <span className="text-4xl mb-2">✅</span>
                <p>No active jobs</p>
              </div>
            ) : (
              <ul className="divide-y divide-border/60">
                {activeJobs.map(job => (
                  <li key={job.id}>
                    <Link 
                      href={`/jobs/${job.id}`}
                      className="block px-4 py-3 hover:bg-accent/50 transition-colors duration-200"
                      onClick={() => setIsOpen(false)}
                    >
                      <div className="flex items-start gap-3">
                        <div className="mt-0.5">
                          {(job.status === "running" || job.status === "generating_thumbnails") && <span className="animate-spin inline-block">⏳</span>}
                          {job.status === "pending" && <span>⏸</span>}
                        </div>
                        
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <code className="text-xs px-1.5 py-0.5 bg-muted rounded font-mono">{job.id.slice(0, 8)}</code>
                            <Badge variant={job.type === 'rebuild' ? 'primary' : job.type === 'thumbnail_regenerate' ? 'warning' : 'secondary'} className="text-[10px]">
                              {job.type === 'thumbnail_rebuild' ? 'Thumbnails' : job.type === 'thumbnail_regenerate' ? 'Regenerate' : job.type}
                            </Badge>
                          </div>
                          
                          {(job.status === "running" || job.status === "generating_thumbnails") && job.progress_percent != null && (
                            <div className="flex items-center gap-2 mt-2">
                              <MiniProgressBar value={job.progress_percent} />
                              <span className="text-xs font-medium text-muted-foreground">{job.progress_percent}%</span>
                            </div>
                          )}
                          
                          {job.current_file && (
                            <p className="text-xs text-muted-foreground mt-1.5 truncate" title={job.current_file}>
                              📄 {job.current_file}
                            </p>
                          )}
                          
                          {job.stats_json && (
                            <div className="flex items-center gap-3 mt-1.5 text-xs text-muted-foreground">
                              <span>✓ {job.stats_json.indexed_files}</span>
                              {job.stats_json.errors > 0 && (
                                <span className="text-destructive">⚠ {job.stats_json.errors}</span>
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
          <div className="px-4 py-2 border-t border-border/60 bg-muted/50">
            <p className="text-xs text-muted-foreground text-center">Auto-refreshing every 2s</p>
          </div>
        </div>
      )}
    </div>
  );
}

// Mini progress bar for dropdown
function MiniProgressBar({ value }: { value: number }) {
  return (
    <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
      <div 
        className="h-full bg-success rounded-full transition-all duration-300"
        style={{ width: `${value}%` }}
      />
    </div>
  );
}

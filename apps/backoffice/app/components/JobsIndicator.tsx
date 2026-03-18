"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import { useTranslation } from "../../lib/i18n/context";
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
    warnings: number;
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
  const { t } = useTranslation();
  const [activeJobs, setActiveJobs] = useState<Job[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popinRef = useRef<HTMLDivElement>(null);
  const [popinStyle, setPopinStyle] = useState<React.CSSProperties>({});

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

  // Position the popin relative to the button
  const updatePosition = useCallback(() => {
    if (!buttonRef.current) return;
    const rect = buttonRef.current.getBoundingClientRect();
    const isMobile = window.innerWidth < 640;

    if (isMobile) {
      setPopinStyle({
        position: "fixed",
        top: `${rect.bottom + 8}px`,
        left: "12px",
        right: "12px",
      });
    } else {
      // Align right edge of popin with right edge of button
      const rightEdge = window.innerWidth - rect.right;
      setPopinStyle({
        position: "fixed",
        top: `${rect.bottom + 8}px`,
        right: `${Math.max(rightEdge, 12)}px`,
        width: "384px", // w-96
      });
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [isOpen, updatePosition]);

  // Close when clicking outside
  useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        buttonRef.current && !buttonRef.current.contains(target) &&
        popinRef.current && !popinRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [isOpen]);

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return;
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("keydown", handleEsc);
    return () => document.removeEventListener("keydown", handleEsc);
  }, [isOpen]);

  const runningJobs = activeJobs.filter(j => j.status === "running" || j.status === "extracting_pages" || j.status === "generating_thumbnails");
  const pendingJobs = activeJobs.filter(j => j.status === "pending");
  const totalCount = activeJobs.length;

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
        title={t("jobsIndicator.viewAll")}
      >
        <JobsIcon className="w-[18px] h-[18px]" />
      </Link>
    );
  }

  const popin = isOpen && (
    <>
      {/* Mobile backdrop */}
      <div
        className="fixed inset-0 z-[80] sm:hidden bg-background/60 backdrop-blur-sm"
        onClick={() => setIsOpen(false)}
        aria-hidden="true"
      />

      {/* Popin */}
      <div
        ref={popinRef}
        style={popinStyle}
        className="
          z-[90]
          bg-popover/95 backdrop-blur-md
          rounded-xl
          shadow-elevation-2
          border border-border/60
          overflow-hidden
          animate-fade-in
        "
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border/60 bg-muted/50">
          <div className="flex items-center gap-3">
            <span className="text-xl">📊</span>
            <div>
              <h3 className="font-semibold text-foreground">{t("jobsIndicator.activeTasks")}</h3>
              <p className="text-xs text-muted-foreground">
                {runningJobs.length > 0
                  ? t("jobsIndicator.runningAndPending", { running: runningJobs.length, pending: pendingJobs.length })
                  : t("jobsIndicator.pendingTasks", { count: pendingJobs.length, plural: pendingJobs.length !== 1 ? "s" : "" })
                }
              </p>
            </div>
          </div>
          <Link
            href="/jobs"
            className="text-sm font-medium text-primary hover:text-primary/80 transition-colors"
            onClick={() => setIsOpen(false)}
          >
            {t("jobsIndicator.viewAllLink")}
          </Link>
        </div>

        {/* Overall progress bar if running */}
        {runningJobs.length > 0 && (
          <div className="px-4 py-3 border-b border-border/60">
            <div className="flex items-center justify-between text-sm mb-2">
              <span className="text-muted-foreground">{t("jobsIndicator.overallProgress")}</span>
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
              <p>{t("jobsIndicator.noActiveTasks")}</p>
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
                        {(job.status === "running" || job.status === "extracting_pages" || job.status === "generating_thumbnails") && <span className="animate-spin inline-block">⏳</span>}
                        {job.status === "pending" && <span>⏸</span>}
                      </div>

                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <code className="text-xs px-1.5 py-0.5 bg-muted rounded font-mono">{job.id.slice(0, 8)}</code>
                          <Badge variant={job.type === 'rebuild' ? 'primary' : job.type === 'thumbnail_regenerate' ? 'warning' : 'secondary'} className="text-[10px]">
                            {t(`jobType.${job.type}` as any) !== `jobType.${job.type}` ? t(`jobType.${job.type}` as any) : job.type}
                          </Badge>
                        </div>

                        {(job.status === "running" || job.status === "extracting_pages" || job.status === "generating_thumbnails") && job.progress_percent != null && (
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
                            {(job.stats_json.warnings ?? 0) > 0 && (
                              <span className="text-warning">⚠ {job.stats_json.warnings}</span>
                            )}
                            {job.stats_json.errors > 0 && (
                              <span className="text-destructive">✕ {job.stats_json.errors}</span>
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
          <p className="text-xs text-muted-foreground text-center">{t("jobsIndicator.autoRefresh")}</p>
        </div>
      </div>
    </>
  );

  return (
    <>
      <button
        ref={buttonRef}
        className={`
          flex items-center gap-1.5
          p-2 sm:px-3 sm:py-2
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
        title={t("jobsIndicator.taskCount", { count: totalCount, plural: totalCount !== 1 ? "s" : "" })}
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

        {/* Chevron - hidden on small screens */}
        <ChevronIcon
          className={`w-4 h-4 hidden sm:block transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {typeof document !== "undefined" && createPortal(popin, document.body)}
    </>
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

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
  const [isLoading, setIsLoading] = useState(false);
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
      <Link href="/jobs" className="jobs-indicator-empty" title="View all jobs">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="2" y="3" width="20" height="18" rx="2" />
          <path d="M6 8h12M6 12h12M6 16h8" />
        </svg>
      </Link>
    );
  }

  return (
    <div className="jobs-indicator-wrapper" ref={dropdownRef}>
      <button 
        className={`jobs-indicator-button ${runningJobs.length > 0 ? 'has-running' : ''} ${isOpen ? 'open' : ''}`}
        onClick={() => setIsOpen(!isOpen)}
        title={`${totalCount} active job${totalCount !== 1 ? 's' : ''}`}
      >
        {/* Animated spinner for running jobs */}
        {runningJobs.length > 0 && (
          <div className="jobs-spinner">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
              <path d="M12 2a10 10 0 0 1 10 10" strokeLinecap="round" />
            </svg>
          </div>
        )}
        
        {/* Icon */}
        <svg className="jobs-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="2" y="3" width="20" height="18" rx="2" />
          <path d="M6 8h12M6 12h12M6 16h8" />
        </svg>
        
        {/* Badge with count */}
        <span className="jobs-count-badge">
          {totalCount > 99 ? "99+" : totalCount}
        </span>
        
        {/* Chevron */}
        <svg 
          className={`jobs-chevron ${isOpen ? 'open' : ''}`} 
          width="16" 
          height="16" 
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
        <div className="jobs-popin">
          <div className="jobs-popin-header">
            <div className="jobs-popin-title">
              <span className="jobs-icon-large">📊</span>
              <div>
                <h3>Active Jobs</h3>
                <p className="jobs-subtitle">
                  {runningJobs.length > 0 
                    ? `${runningJobs.length} running, ${pendingJobs.length} pending`
                    : `${pendingJobs.length} job${pendingJobs.length !== 1 ? 's' : ''} pending`
                  }
                </p>
              </div>
            </div>
            <Link 
              href="/jobs" 
              className="jobs-view-all"
              onClick={() => setIsOpen(false)}
            >
              View All →
            </Link>
          </div>

          {/* Overall progress bar if running */}
          {runningJobs.length > 0 && (
            <div className="jobs-overall-progress">
              <div className="progress-header">
                <span>Overall Progress</span>
                <span className="progress-percent">{Math.round(totalProgress)}%</span>
              </div>
              <div className="progress-bar">
                <div 
                  className="progress-fill" 
                  style={{ width: `${totalProgress}%` }}
                />
              </div>
            </div>
          )}
          
          <div className="jobs-list-container">
            {activeJobs.length === 0 ? (
              <div className="jobs-empty-state">
                <span className="empty-icon">✅</span>
                <p>No active jobs</p>
              </div>
            ) : (
              <ul className="jobs-detailed-list">
                {activeJobs.map(job => (
                  <li key={job.id} className={`job-detailed-item job-status-${job.status}`}>
                    <Link 
                      href={`/jobs/${job.id}`}
                      className="job-link"
                      onClick={() => setIsOpen(false)}
                    >
                      <div className="job-info-row">
                        <div className="job-status-icon">
                          {job.status === "running" && <span className="spinning">⏳</span>}
                          {job.status === "pending" && <span>⏸</span>}
                        </div>
                        
                        <div className="job-details">
                          <div className="job-main-info">
                            <code className="job-id-short">{job.id.slice(0, 8)}</code>
                            <span className={`job-type-badge ${job.type}`}>{job.type}</span>
                          </div>
                          
                          {job.status === "running" && job.progress_percent !== null && (
                            <div className="job-progress-row">
                              <div className="job-mini-progress-bar">
                                <div 
                                  className="job-mini-progress-fill" 
                                  style={{ width: `${job.progress_percent}%` }}
                                />
                              </div>
                              <span className="job-progress-text">{job.progress_percent}%</span>
                            </div>
                          )}
                          
                          {job.current_file && (
                            <p className="job-current-file" title={job.current_file}>
                              📄 {job.current_file.length > 35 
                                ? job.current_file.substring(0, 35) + "..." 
                                : job.current_file}
                            </p>
                          )}
                          
                          {job.stats_json && (
                            <div className="job-mini-stats">
                              <span>✓ {job.stats_json.indexed_files}</span>
                              {job.stats_json.errors > 0 && (
                                <span className="error-stat">⚠ {job.stats_json.errors}</span>
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
          <div className="jobs-popin-footer">
            <p className="jobs-auto-refresh">Auto-refreshing every 2s</p>
          </div>
        </div>
      )}
    </div>
  );
}

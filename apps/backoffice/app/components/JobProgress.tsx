"use client";

import { useEffect, useState } from "react";

interface ProgressEvent {
  job_id: string;
  status: string;
  current_file: string | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
  } | null;
}

interface JobProgressProps {
  jobId: string;
  onComplete?: () => void;
}

export function JobProgress({ jobId, onComplete }: JobProgressProps) {
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    // Use SSE via local proxy
    const eventSource = new EventSource(`/api/jobs/${jobId}/stream`);
    
    eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        
        const progressData: ProgressEvent = {
          job_id: data.id,
          status: data.status,
          current_file: data.current_file,
          progress_percent: data.progress_percent,
          processed_files: data.processed_files,
          total_files: data.total_files,
          stats_json: data.stats_json,
        };

        setProgress(progressData);

        if (data.status === "success" || data.status === "failed" || data.status === "cancelled") {
          setIsComplete(true);
          eventSource.close();
          onComplete?.();
        }
      } catch (err) {
        setError("Failed to parse SSE data");
      }
    };
    
    eventSource.onerror = (err) => {
      console.error("SSE error:", err);
      eventSource.close();
      setError("Connection lost");
    };

    return () => {
      eventSource.close();
    };
  }, [jobId, onComplete]);

  if (error) {
    return <div className="progress-error">Error: {error}</div>;
  }

  if (!progress) {
    return <div className="progress-loading">Loading progress...</div>;
  }

  const percent = progress.progress_percent ?? 0;
  const processed = progress.processed_files ?? 0;
  const total = progress.total_files ?? 0;

  return (
    <div className="job-progress">
      <div className="progress-header">
        <span className={`status-badge status-${progress.status}`}>
          {progress.status}
        </span>
        {isComplete && <span className="complete-badge">Complete</span>}
      </div>

      <div className="progress-bar-container">
        <div 
          className="progress-bar-fill" 
          style={{ width: `${percent}%` }}
        />
        <span className="progress-percent">{percent}%</span>
      </div>

      <div className="progress-stats">
        <span>{processed} / {total} files</span>
        {progress.current_file && (
          <span className="current-file" title={progress.current_file}>
            Current: {progress.current_file.length > 40 
              ? progress.current_file.substring(0, 40) + "..." 
              : progress.current_file}
          </span>
        )}
      </div>

      {progress.stats_json && (
        <div className="progress-detailed-stats">
          <span>Scanned: {progress.stats_json.scanned_files}</span>
          <span>Indexed: {progress.stats_json.indexed_files}</span>
          <span>Removed: {progress.stats_json.removed_files}</span>
          {progress.stats_json.errors > 0 && (
            <span className="error-count">Errors: {progress.stats_json.errors}</span>
          )}
        </div>
      )}
    </div>
  );
}

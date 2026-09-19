import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { JobProgress } from "@/app/components/JobProgress";

class FakeEventSource {
  static instances: FakeEventSource[] = [];

  url: string;
  closed = false;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close() {
    this.closed = true;
  }

  emit(data: string) {
    this.onmessage?.({ data });
  }

  fail() {
    this.onerror?.();
  }
}

function latest() {
  return FakeEventSource.instances[FakeEventSource.instances.length - 1];
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("JobProgress", () => {
  it("shows a loading state and opens the job stream", () => {
    render(<JobProgress jobId="job-1" />);

    expect(screen.getByText("jobProgress.loadingProgress")).toBeInTheDocument();
    expect(latest().url).toBe("/api/jobs/job-1/stream");
  });

  it("renders progress, counters, current file and stats while running", () => {
    render(<JobProgress jobId="job-1" />);

    act(() =>
      latest().emit(
        JSON.stringify({
          id: "job-1",
          status: "running",
          current_file: "a/very/long/path/that/exceeds/the/forty/character/limit.cbz",
          progress_percent: 42,
          processed_files: 5,
          total_files: 10,
          stats_json: { scanned_files: 7, indexed_files: 6, removed_files: 1, errors: 2 },
        })
      )
    );

    expect(screen.getByText(/5 \/ 10/)).toBeInTheDocument();
    expect(screen.getByText(/42%/)).toBeInTheDocument();
    expect(screen.getByText(/\.\.\./)).toBeInTheDocument();
    expect(screen.getByText(/jobProgress\.scanned/)).toBeInTheDocument();
    expect(screen.getByText(/jobProgress\.indexed/)).toBeInTheDocument();
    expect(screen.getByText(/jobProgress\.removed/)).toBeInTheDocument();
    expect(screen.getByText(/jobProgress\.errors/)).toBeInTheDocument();
  });

  it("uses a page unit and hides stats during phase 2", () => {
    render(<JobProgress jobId="job-1" />);

    act(() =>
      latest().emit(
        JSON.stringify({
          id: "job-1",
          status: "extracting_pages",
          current_file: null,
          progress_percent: 10,
          processed_files: 1,
          total_files: 10,
          stats_json: { scanned_files: 7, indexed_files: 6, removed_files: 1, errors: 0 },
        })
      )
    );

    expect(screen.getByText(/jobProgress\.pages/)).toBeInTheDocument();
    expect(screen.queryByText(/jobProgress\.scanned/)).not.toBeInTheDocument();
  });

  it("marks the job done, closes the stream and notifies the parent on success", () => {
    const onComplete = vi.fn();
    render(<JobProgress jobId="job-1" onComplete={onComplete} />);

    act(() =>
      latest().emit(
        JSON.stringify({
          id: "job-1",
          status: "success",
          current_file: null,
          progress_percent: 100,
          processed_files: 10,
          total_files: 10,
          stats_json: null,
        })
      )
    );

    expect(screen.getByText("jobProgress.done")).toBeInTheDocument();
    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(latest().closed).toBe(true);
  });

  it("reports a malformed payload", () => {
    render(<JobProgress jobId="job-1" />);

    act(() => latest().emit("{"));

    expect(screen.getByText(/jobProgress\.sseError/)).toBeInTheDocument();
  });

  it("reports a lost connection", () => {
    render(<JobProgress jobId="job-1" />);

    act(() => latest().fail());

    expect(screen.getByText(/jobProgress\.connectionLost/)).toBeInTheDocument();
  });
});

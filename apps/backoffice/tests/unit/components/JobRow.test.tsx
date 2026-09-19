import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { JobRow } from "@/app/components/JobRow";

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

  emit(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) });
  }
}

function latest() {
  return FakeEventSource.instances[FakeEventSource.instances.length - 1];
}

type Job = Parameters<typeof JobRow>[0]["job"];

function makeJob(overrides: Partial<Job> = {}): Job {
  return {
    id: "abcdef1234567890",
    library_id: "lib-1",
    type: "scan",
    status: "success",
    created_at: "2024-01-01T10:00:00Z",
    started_at: "2024-01-01T10:00:00Z",
    finished_at: "2024-01-01T10:05:00Z",
    error_opt: null,
    stats_json: { scanned_files: 10, indexed_files: 8, removed_files: 2, errors: 0 },
    progress_percent: 100,
    processed_files: 10,
    total_files: 10,
    current_file: null,
    ...overrides,
  };
}

function renderRow(overrides: Partial<Job> = {}, props: Record<string, unknown> = {}) {
  const onCancel = vi.fn();
  const onReplay = vi.fn();
  const utils = render(
    <table>
      <tbody>
        <JobRow
          job={makeJob(overrides)}
          libraryName="My Library"
          onCancel={onCancel}
          onReplay={onReplay}
          formatDate={(d) => `date:${d}`}
          formatDuration={(start, end) => `dur:${start}:${end}`}
          {...props}
        />
      </tbody>
    </table>
  );
  return { ...utils, onCancel, onReplay };
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("JobRow", () => {
  it("renders the job identity, library and type", () => {
    renderRow();

    expect(screen.getByRole("link", { name: "abcdef12" })).toHaveAttribute(
      "href",
      "/jobs/abcdef1234567890"
    );
    expect(screen.getByText("My Library")).toBeInTheDocument();
    expect(screen.getByText("jobType.scan")).toBeInTheDocument();
    expect(screen.getByText("success")).toBeInTheDocument();
  });

  it("falls back to the truncated library id or a dash", () => {
    const { rerender } = renderRow({ library_id: "library-xyz" }, { libraryName: undefined });
    expect(screen.getByText("library-")).toBeInTheDocument();

    rerender(
      <table>
        <tbody>
          <JobRow
            job={makeJob({ library_id: null, stats_json: null, total_files: null })}
            libraryName={undefined}
            onCancel={vi.fn()}
            onReplay={vi.fn()}
            formatDate={(d) => d}
            formatDuration={() => "-"}
          />
        </tbody>
      </table>
    );
    expect(screen.getAllByText("—")).toHaveLength(2);
  });

  it("renders the duration and formatted creation date", () => {
    renderRow();

    expect(screen.getByText("dur:2024-01-01T10:00:00Z:2024-01-01T10:05:00Z")).toBeInTheDocument();
    expect(screen.getByText("date:2024-01-01T10:00:00Z")).toBeInTheDocument();
  });

  it("shows completed stats and the error indicator", () => {
    renderRow({
      stats_json: { scanned_files: 10, indexed_files: 8, removed_files: 2, errors: 3 },
      error_opt: "boom",
    });

    expect(screen.getByText('jobRow.filesIndexed:{"count":8}')).toBeInTheDocument();
    expect(screen.getByText('jobRow.filesRemoved:{"count":2}')).toBeInTheDocument();
    expect(screen.getByText('jobRow.errors:{"count":3}')).toBeInTheDocument();
    expect(screen.getByTitle("boom")).toBeInTheDocument();
  });

  it("shows scanned count only when nothing else is present", () => {
    renderRow({
      type: "some_type",
      stats_json: { scanned_files: 5, indexed_files: 0, removed_files: 0, errors: 0 },
    });

    expect(screen.getByText('jobRow.scanned:{"count":5}')).toBeInTheDocument();
  });

  it("renders a dash when there are no stats at all", () => {
    renderRow({ type: "some_type", stats_json: null, total_files: null });
    expect(screen.getByText("—")).toBeInTheDocument();
  });

  it("shows live progress for an active job and allows toggling it", () => {
    renderRow({ status: "running", stats_json: null, processed_files: 4, total_files: 10 });

    expect(screen.getByText("4/10")).toBeInTheDocument();
    expect(latest().url).toBe("/api/jobs/abcdef1234567890/stream");
    expect(document.querySelector('tr td[colspan="8"]')).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "jobRow.hideProgress" }));
    expect(screen.getByRole("button", { name: "jobRow.showProgress" })).toBeInTheDocument();
    expect(document.querySelector('tr td[colspan="8"]')).toBeNull();
  });

  it("hides progress when the stream reports completion", () => {
    renderRow({ status: "running", stats_json: null });

    expect(document.querySelector('tr td[colspan="8"]')).toBeInTheDocument();
    act(() =>
      latest().emit({
        id: "abcdef1234567890",
        status: "success",
        current_file: null,
        progress_percent: 100,
        processed_files: 10,
        total_files: 10,
        stats_json: null,
      })
    );
    expect(document.querySelector('tr td[colspan="8"]')).toBeNull();
  });

  it("cancels an active job", () => {
    const { onCancel } = renderRow({ status: "pending", stats_json: null });

    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));

    expect(onCancel).toHaveBeenCalledWith("abcdef1234567890");
  });

  it("replays a finished replayable job", () => {
    const { onReplay } = renderRow({ type: "rescan", status: "success" });

    fireEvent.click(screen.getByRole("button", { name: "jobRow.replay" }));

    expect(onReplay).toHaveBeenCalledWith("abcdef1234567890");
  });

  it("does not offer replay for a non-replayable job", () => {
    renderRow({ type: "rating_pull", status: "success" });
    expect(screen.queryByRole("button", { name: "jobRow.replay" })).toBeNull();
  });

  it("renders thumbnail stats for a thumbnail job", () => {
    renderRow({
      type: "thumbnail_rebuild",
      status: "success",
      stats_json: { scanned_files: 0, indexed_files: 0, removed_files: 0, errors: 0 },
      total_files: 4,
    });

    expect(screen.getByText('jobRow.thumbnailsGenerated:{"count":4}')).toBeInTheDocument();
  });
});

describe("JobRow highlight", () => {
  it("marks a highlighted job and auto-expands its progress", () => {
    const { container } = renderRow(
      { status: "running", stats_json: null },
      { highlighted: true }
    );

    expect(container.querySelector("tr")).toHaveClass("bg-primary/10");
    expect(document.querySelector('tr td[colspan="8"]')).toBeInTheDocument();
  });
});

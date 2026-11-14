import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useJobsStream, useJobStream, type JobStreamEvent } from "@/lib/jobs/useJobStream";

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

  emitRaw(data: string) {
    this.onmessage?.({ data });
  }

  fail() {
    this.onerror?.();
  }
}

function latest() {
  return FakeEventSource.instances[FakeEventSource.instances.length - 1];
}

function jobEvent(overrides: Partial<JobStreamEvent> = {}): JobStreamEvent {
  return {
    id: "job-1",
    status: "running",
    current_file: null,
    progress_percent: 42,
    processed_files: 5,
    total_files: 10,
    stats_json: { scanned_files: 7, indexed_files: 6, removed_files: 1, errors: 0 },
    ...overrides,
  };
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useJobStream", () => {
  it("opens the job stream and exposes the payload", () => {
    const { result } = renderHook(() => useJobStream("job-1"));

    expect(latest().url).toBe("/api/jobs/job-1/stream");
    expect(result.current.job).toBeNull();

    act(() => latest().emit(jobEvent()));

    expect(result.current.job).toMatchObject({ status: "running", progress_percent: 42 });
    expect(result.current.isComplete).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("notifies onMessage for every update", () => {
    const onMessage = vi.fn();
    renderHook(() => useJobStream("job-1", { onMessage }));

    const event = jobEvent();
    act(() => latest().emit(event));

    expect(onMessage).toHaveBeenCalledWith(event);
  });

  it("closes the stream once, when the job becomes terminal", () => {
    const onComplete = vi.fn();
    const { result } = renderHook(() => useJobStream("job-1", { onComplete }));

    const source = latest();
    act(() => latest().emit(jobEvent({ status: "success" })));

    expect(result.current.isComplete).toBe(true);
    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(source.closed).toBe(true);
    expect(FakeEventSource.instances).toHaveLength(1);
  });

  it("does not open a stream while disabled", () => {
    renderHook(() => useJobStream("job-1", { enabled: false }));

    expect(FakeEventSource.instances).toHaveLength(0);
  });

  it("opens the stream when it becomes enabled", () => {
    const { rerender } = renderHook(
      ({ enabled }) => useJobStream("job-1", { enabled }),
      { initialProps: { enabled: false } }
    );

    expect(FakeEventSource.instances).toHaveLength(0);

    rerender({ enabled: true });

    expect(FakeEventSource.instances).toHaveLength(1);
    expect(latest().url).toBe("/api/jobs/job-1/stream");
  });

  it("reports a lost connection, then clears the error on the next update", () => {
    const { result } = renderHook(() => useJobStream("job-1"));

    act(() => latest().fail());
    expect(result.current.error).toBe("connection");

    act(() => latest().emit(jobEvent()));
    expect(result.current.error).toBeNull();
  });

  it("reports malformed payloads", () => {
    const { result } = renderHook(() => useJobStream("job-1"));

    act(() => latest().emitRaw("{"));

    expect(result.current.error).toBe("parse");
  });

  it("closes the connection on unmount", () => {
    const { unmount } = renderHook(() => useJobStream("job-1"));
    const source = latest();

    unmount();

    expect(source.closed).toBe(true);
  });
});

describe("useJobsStream", () => {
  it("starts from the server-rendered jobs and refreshes on list updates", () => {
    const { result } = renderHook(() => useJobsStream({ initial: [{ id: "a" }] }));

    expect(latest().url).toBe("/api/jobs/stream");
    expect(result.current.jobs).toEqual([{ id: "a" }]);

    act(() => latest().emit([{ id: "b" }, { id: "c" }]));

    expect(result.current.jobs).toEqual([{ id: "b" }, { id: "c" }]);
  });

  it("ignores payloads that are not a job list", () => {
    const { result } = renderHook(() => useJobsStream({ initial: [{ id: "a" }] }));

    act(() => latest().emit({ error: "nope" }));

    expect(result.current.jobs).toEqual([{ id: "a" }]);
  });

  it("exposes the setter so callers can patch the list locally", () => {
    const { result } = renderHook(() => useJobsStream({ initial: [{ id: "a" }] }));

    act(() => result.current.setJobs([{ id: "z" }]));

    expect(result.current.jobs).toEqual([{ id: "z" }]);
  });

  it("does not open a stream while disabled", () => {
    renderHook(() => useJobsStream({ initial: [], enabled: false }));

    expect(FakeEventSource.instances).toHaveLength(0);
  });
});

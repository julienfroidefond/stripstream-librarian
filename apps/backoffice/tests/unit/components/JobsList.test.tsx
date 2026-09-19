import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { JobsList } from "@/app/components/JobsList";

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

type Job = Parameters<typeof JobsList>[0]["initialJobs"][number];

function makeJob(id: string, overrides: Partial<Job> = {}): Job {
  return {
    id,
    library_id: "l1",
    type: "scan",
    status: "success",
    created_at: "2024-01-01T10:00:00Z",
    started_at: "2024-01-01T10:00:00Z",
    finished_at: "2024-01-01T10:05:00Z",
    error_opt: null,
    stats_json: { scanned_files: 10, indexed_files: 8, removed_files: 0, errors: 0 },
    progress_percent: 100,
    processed_files: 10,
    total_files: 10,
    current_file: null,
    ...overrides,
  };
}

const libraries = new Map([
  ["l1", "Library One"],
  ["l2", "Library Two"],
]);

function stubFetch(listPayload: Job[] = []) {
  const fetchMock = vi.fn((url: string) => {
    const u = String(url);
    if (u === "/api/jobs/list") return Promise.resolve({ ok: true, json: async () => listPayload });
    return Promise.resolve({ ok: true, json: async () => ({}) });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("JobsList", () => {
  it("renders the header row and every job", () => {
    stubFetch();
    render(
      <JobsList
        initialJobs={[
          makeJob("aaaaaaaa1111"),
          makeJob("bbbbbbbb2222", { type: "rebuild", status: "running", library_id: "l2" }),
        ]}
        libraries={libraries}
      />
    );

    expect(screen.getByText("jobsList.id")).toBeInTheDocument();
    expect(screen.getByText("jobsList.actions")).toBeInTheDocument();
    expect(screen.getByText("aaaaaaaa")).toBeInTheDocument();
    expect(screen.getByText("bbbbbbbb")).toBeInTheDocument();
    expect(screen.getAllByText("Library One").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Library Two").length).toBeGreaterThan(0);
    expect(screen.getByText("2 / 2")).toBeInTheDocument();
  });

  it("filters jobs by type, status and library, then clears", () => {
    stubFetch();
    render(
      <JobsList
        initialJobs={[
          makeJob("aaaaaaaa1111"),
          makeJob("bbbbbbbb2222", { type: "rebuild", status: "running" }),
          makeJob("cccccccc3333", { status: "failed", library_id: "l2" }),
        ]}
        libraries={libraries}
      />
    );

    const [typeFilter, statusFilter, libraryFilter] = screen.getAllByRole("combobox");

    fireEvent.change(typeFilter, { target: { value: "scan" } });
    expect(screen.getByText("2 / 3")).toBeInTheDocument();
    expect(screen.queryByText("bbbbbbbb")).toBeNull();

    fireEvent.change(statusFilter, { target: { value: "failed" } });
    expect(screen.getByText("1 / 3")).toBeInTheDocument();

    fireEvent.change(statusFilter, { target: { value: "" } });
    fireEvent.change(libraryFilter, { target: { value: "l2" } });
    expect(screen.getByText("1 / 3")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "common.clear" }));
    expect(screen.getByText("3 / 3")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "common.clear" })).toBeNull();
  });

  it("refreshes the list from the SSE stream", () => {
    stubFetch();
    render(<JobsList initialJobs={[makeJob("aaaaaaaa1111")]} libraries={libraries} />);

    act(() => FakeEventSource.instances[0].emit([makeJob("dddddddd4444", { library_id: null })]));

    expect(screen.getByText("dddddddd")).toBeInTheDocument();
    expect(screen.getByText("1 / 1")).toBeInTheDocument();
  });

  it("marks a job cancelled after a successful cancel request", async () => {
    const fetchMock = stubFetch();
    render(
      <JobsList
        initialJobs={[makeJob("aaaaaaaa1111", { status: "running" })]}
        libraries={libraries}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));

    expect(await screen.findAllByText("cancelled")).toHaveLength(2);
    expect(fetchMock).toHaveBeenCalledWith("/api/jobs/aaaaaaaa1111/cancel", { method: "POST" });
    expect(screen.queryByRole("button", { name: "common.cancel" })).toBeNull();
  });

  it("reloads the list after a replay", async () => {
    const fetchMock = stubFetch([makeJob("eeeeeeee5555")]);
    render(
      <JobsList
        initialJobs={[makeJob("aaaaaaaa1111", { type: "rescan" })]}
        libraries={libraries}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "jobRow.replay" }));

    await screen.findByText("eeeeeeee");
    expect(fetchMock).toHaveBeenCalledWith("/api/jobs/aaaaaaaa1111/replay", { method: "POST" });
    expect(fetchMock).toHaveBeenCalledWith("/api/jobs/list");
  });

  it("paginates when there are more than 25 jobs", () => {
    stubFetch();
    const jobs = Array.from({ length: 30 }, (_, i) =>
      makeJob(`${String(i).padStart(8, "0")}xxxx`)
    );
    render(<JobsList initialJobs={jobs} libraries={libraries} />);

    expect(screen.getByText('pagination.range:{"start":1,"end":25,"total":30}')).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "pagination.previous" })).toBeDisabled();
    expect(screen.getByText("00000024")).toBeInTheDocument();
    expect(screen.queryByText("00000025")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "pagination.next" }));
    expect(screen.getByText('pagination.range:{"start":26,"end":30,"total":30}')).toBeInTheDocument();
    expect(screen.getByText("00000025")).toBeInTheDocument();
    expect(screen.queryByText("00000024")).toBeNull();
  });

  it("opens on the page containing a highlighted job", () => {
    stubFetch();
    const jobs = Array.from({ length: 30 }, (_, i) =>
      makeJob(`${String(i).padStart(8, "0")}xxxx`)
    );
    render(<JobsList initialJobs={jobs} libraries={libraries} highlightJobId={jobs[26].id} />);

    expect(screen.getByText('pagination.range:{"start":26,"end":30,"total":30}')).toBeInTheDocument();
    expect(screen.getByText("00000026")).toBeInTheDocument();
  });
});

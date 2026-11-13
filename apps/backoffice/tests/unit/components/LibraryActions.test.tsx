import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { refresh } = vi.hoisted(() => ({ refresh: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh, push: vi.fn(), replace: vi.fn(), back: vi.fn(), prefetch: vi.fn() }),
}));

import { LibraryActions } from "@/app/components/LibraryActions";
import type { LibraryDto, MetadataProviderDto } from "@/lib/api";

const library: LibraryDto = {
  id: "lib-1",
  name: "Manga",
  root_path: "/libraries/manga",
  enabled: true,
  book_count: 10,
  monitor_enabled: true,
  scan_mode: "daily",
  next_scan_at: null,
  watcher_enabled: false,
  metadata_provider: "anilist",
  fallback_metadata_provider: null,
  metadata_refresh_mode: "manual",
  next_metadata_refresh_at: null,
  series_count: 2,
  thumbnail_book_ids: [],
  reading_status_provider: null,
  reading_status_push_mode: "manual",
  next_reading_status_push_at: null,
  download_detection_mode: "manual",
  next_download_detection_at: null,
  tags: ["manga"],
};

const providers: MetadataProviderDto[] = [
  { id: "anilist", label: "AniList", requires_api_key: false },
  { id: "google_books", label: "Google Books", requires_api_key: true },
];

function stubFetch(failOn?: string) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    void init;
    if (failOn && String(url).endsWith(failOn)) {
      return new Response(JSON.stringify({ error: "upstream refused" }), {
        status: 500,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response(JSON.stringify(library), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
  refresh.mockReset();
});

function open() {
  render(<LibraryActions library={library} providers={providers} />);
  fireEvent.click(screen.getByRole("button", { name: "libraryActions.settingsTitle" }));
}

function submit() {
  fireEvent.click(screen.getByRole("button", { name: "common.save" }));
}

describe("LibraryActions", () => {
  it("lists the providers given by the page without fetching them", () => {
    const fetchMock = stubFetch();
    open();
    expect(screen.getAllByRole("option", { name: "Google Books" })).toHaveLength(2);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("only sends the sections that changed, then refreshes the router", async () => {
    const fetchMock = stubFetch();
    open();

    fireEvent.click(screen.getByRole("button", { name: "webtoon" }));
    submit();

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/libraries/lib-1/tags");
    expect(JSON.parse(init!.body as string)).toEqual({ tags: ["manga", "webtoon"] });
    await waitFor(() => expect(screen.queryByTestId("library-settings-panel")).toBeNull());
  });

  it("closes without writing when nothing changed", async () => {
    const fetchMock = stubFetch();
    open();
    submit();

    await waitFor(() => expect(screen.queryByTestId("library-settings-panel")).toBeNull());
    expect(fetchMock).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
  });

  it("stops at the first failing write, keeps the modal open and shows the error", async () => {
    const fetchMock = stubFetch("/reading-status-provider");
    open();

    fireEvent.change(screen.getByDisplayValue("AniList"), { target: { value: "google_books" } });
    const [, , , , readingStatus] = screen.getAllByRole("combobox");
    fireEvent.change(readingStatus, { target: { value: "anilist" } });
    fireEvent.click(screen.getByRole("button", { name: "comics" }));
    submit();

    expect(await screen.findByText("upstream refused")).toBeInTheDocument();
    const urls = fetchMock.mock.calls.map(([url]) => url);
    expect(urls).toEqual([
      "/api/libraries/lib-1/metadata-provider",
      "/api/libraries/lib-1/reading-status-provider",
    ]);
    // The metadata write went through: the page is refreshed to show it.
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("library-settings-panel")).toBeInTheDocument();
  });
});

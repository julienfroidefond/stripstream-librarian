import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AvailableDownloadsSection } from "@/app/(app)/downloads/DownloadsPage";
import { DownloadActionsProvider } from "@/app/(app)/downloads/useDownloadActions";
import type { LatestFoundPerLibraryDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function ok(payload: unknown = null) {
  return { ok: true, status: 200, json: async () => payload, text: async () => "" };
}

function ddlRelease(overrides: Record<string, unknown> = {}) {
  return {
    id: "entry-1",
    series_id: "series-1",
    series_name: "Astérix",
    missing_count: 1,
    available_releases: [
      {
        title: "Astérix - T41.pdf",
        size: 1024,
        download_url: null,
        indexer: null,
        seeders: null,
        matched_missing_volumes: [41],
        all_volumes: [41],
        provider: "fourtoutici",
        external_id: "ext-41",
        ...overrides,
      },
    ],
    updated_at: "2026-01-01T00:00:00Z",
    failed_download_count: 0,
  };
}

function latestFound(release: ReturnType<typeof ddlRelease>): LatestFoundPerLibraryDto[] {
  return [{ library_id: "lib-1", library_name: "BD", results: [release] }];
}

function expandSeries(name: string) {
  const summary = screen
    .getAllByRole("button")
    .find((button) => button.textContent?.includes(name));
  if (!summary) throw new Error(`summary row for ${name} not found`);
  fireEvent.click(summary);
}

describe("AvailableDownloadsSection — DDL releases", () => {
  it("labels DDL releases with their provider instead of Prowlarr", async () => {
    render(
      <DownloadActionsProvider onRefresh={() => {}}>
        <AvailableDownloadsSection
          latestFound={latestFound(ddlRelease() as never)}
          telegramAvailable={[]}
        />
      </DownloadActionsProvider>
    );

    expandSeries("Astérix");

    expect(await screen.findByText("fourtoutici")).toBeInTheDocument();
    expect(screen.queryByText("Prowlarr")).not.toBeInTheDocument();
  });

  it("starts a DDL download targeting the browsed series", async () => {
    fetchMock.mockResolvedValue(ok({ id: "dl-1", status: "queued" }));
    const onRefresh = vi.fn();

    render(
      <DownloadActionsProvider onRefresh={onRefresh}>
        <AvailableDownloadsSection
          latestFound={latestFound(ddlRelease() as never)}
          telegramAvailable={[]}
        />
      </DownloadActionsProvider>
    );

    expandSeries("Astérix");
    fireEvent.click(await screen.findByTitle("ddlProviders.download"));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/ddl-providers/fourtoutici/downloads",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({
            external_id: "ext-41",
            filename: "Astérix - T41.pdf",
            size: 1024,
            library_id: "lib-1",
            series_name: "Astérix",
            volume_number: 41,
          }),
        })
      )
    );
    await waitFor(() => expect(onRefresh).toHaveBeenCalled());
  });

  it("keeps Prowlarr releases on the Prowlarr label", async () => {
    render(
      <DownloadActionsProvider onRefresh={() => {}}>
        <AvailableDownloadsSection
          latestFound={latestFound(
            ddlRelease({
              provider: null,
              external_id: null,
              download_url: "https://indexer/download",
              indexer: "IndexerA",
            }) as never
          )}
          telegramAvailable={[]}
        />
      </DownloadActionsProvider>
    );

    expandSeries("Astérix");

    expect(await screen.findByText("Prowlarr")).toBeInTheDocument();
    expect(screen.queryByText("fourtoutici")).not.toBeInTheDocument();
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AcquisitionReadinessProvider } from "@/app/components/AcquisitionReadinessProvider";
import { ProwlarrSearchModal } from "@/app/components/ProwlarrSearchModal";
import type { AcquisitionReadiness } from "@/lib/acquisition/readiness";

const fetchMock = vi.fn();

const readyReadiness: AcquisitionReadiness = {
  downloads: true,
  prowlarr: true,
  qbittorrent: false,
  alldebrid: false,
  telegram: false,
  ddl: false,
};

beforeEach(() => {
  fetchMock.mockReset();
  fetchMock.mockImplementation(async () => ok({}));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function ok(payload: unknown) {
  return { ok: true, status: 200, json: async () => payload };
}

function mockSearch(payload: unknown) {
  fetchMock.mockImplementation(async (url: string) => {
    if (url === "/api/prowlarr/search") return ok(payload);
    return ok({});
  });
}

function renderAndOpen(readiness: AcquisitionReadiness = readyReadiness) {
  render(
    <ProwlarrSearchModal
      seriesName="Astérix"
      missingBooks={null}
      initialReadiness={readiness}
    >
      {(open) => <button onClick={open}>open</button>}
    </ProwlarrSearchModal>
  );
  fireEvent.click(screen.getByRole("button", { name: "open" }));
}

function expectNoSettingsFetch() {
  const urls = fetchMock.mock.calls.map(([url]) => String(url));
  expect(
    urls.some(
      (url) =>
        url === "/api/settings/downloads_enabled" ||
        url === "/api/settings/qbittorrent" ||
        url === "/api/settings/alldebrid"
    )
  ).toBe(false);
}

describe("ProwlarrSearchModal", () => {
  it("shows the indexers-not-synced hint when Prowlarr returns no results", async () => {
    mockSearch({
      results: [],
      query: '"Astérix"',
      warning: "indexers_not_synced",
    });
    renderAndOpen();

    expect(await screen.findByText("prowlarr.indexersNotSynced")).toBeInTheDocument();
    expect(screen.getByText("prowlarr.noResults")).toBeInTheDocument();
    expectNoSettingsFetch();
  });

  it("does not show the hint when Prowlarr returns results", async () => {
    mockSearch({
      results: [
        {
          guid: "guid-1",
          title: "Astérix T01",
          size: 100,
          downloadUrl: null,
          indexer: null,
          seeders: null,
          leechers: null,
          publishDate: null,
          protocol: null,
          infoUrl: null,
          categories: null,
          matchedMissingVolumes: null,
        },
      ],
      query: '"Astérix"',
    });
    renderAndOpen();

    expect(await screen.findByText("Astérix T01")).toBeInTheDocument();
    expect(screen.queryByText("prowlarr.indexersNotSynced")).not.toBeInTheDocument();
    expectNoSettingsFetch();
  });

  it("hides the action when Prowlarr is not ready", () => {
    render(
      <ProwlarrSearchModal
        seriesName="Astérix"
        missingBooks={null}
        initialReadiness={{ ...readyReadiness, prowlarr: false }}
      >
        {(open) => <button onClick={open}>open</button>}
      </ProwlarrSearchModal>
    );

    expect(screen.queryByRole("button", { name: "open" })).not.toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("consumes the readiness provided by the app layout", () => {
    render(
      <AcquisitionReadinessProvider readiness={readyReadiness}>
        <ProwlarrSearchModal seriesName="Astérix" missingBooks={null}>
          {(open) => <button onClick={open}>open</button>}
        </ProwlarrSearchModal>
      </AcquisitionReadinessProvider>
    );

    expect(screen.getByRole("button", { name: "open" })).toBeInTheDocument();
    expectNoSettingsFetch();
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ProwlarrSearchModal } from "@/app/components/ProwlarrSearchModal";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
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

function renderAndOpen() {
  render(
    <ProwlarrSearchModal
      seriesName="Astérix"
      missingBooks={null}
      initialProwlarrConfigured
      initialQbConfigured={false}
      initialAlldebridConfigured={false}
    >
      {(open) => <button onClick={open}>open</button>}
    </ProwlarrSearchModal>
  );
  fireEvent.click(screen.getByRole("button", { name: "open" }));
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
  });
});

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { MetadataSearchModal } from "@/app/components/MetadataSearchModal";
import type { ExternalMetadataLinkDto } from "@/lib/api";

function makeLink(overrides: Partial<ExternalMetadataLinkDto> = {}): ExternalMetadataLinkDto {
  return {
    id: "link-1",
    library_id: "lib-1",
    series_name: "Series",
    provider: "bdtheque",
    external_id: "ext-1",
    external_url: "https://example.test/series",
    status: "approved",
    is_primary: false,
    confidence: null,
    metadata_json: {},
    total_volumes_external: null,
    matched_at: "2024-01-01T00:00:00.000Z",
    approved_at: "2024-01-01T00:00:00.000Z",
    synced_at: null,
    ...overrides,
  };
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    const url = typeof input === "string" ? input : input.toString();
    void url;
    return {
      ok: true,
      json: async () => [],
    } as unknown as Response;
  });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function renderModal(links: ExternalMetadataLinkDto[]) {
  render(
    <MetadataSearchModal
      libraryId="lib-1"
      seriesName="Series"
      existingLink={links.find((l) => l.is_primary) ?? links[0] ?? null}
      links={links}
      initialMissing={null}
    >
      {(open) => <button onClick={open}>open</button>}
    </MetadataSearchModal>,
  );
}

describe("MetadataSearchModal primary provider", () => {
  it("lists every approved link and marks the primary one", async () => {
    renderModal([
      makeLink({ id: "a", provider: "bdtheque", is_primary: true }),
      makeLink({ id: "b", provider: "bdphile", is_primary: false }),
    ]);

    fireEvent.click(screen.getByRole("button", { name: "open" }));

    expect(await screen.findByText("metadata.primary")).toBeInTheDocument();
    expect(screen.getByText("metadata.setPrimary")).toBeInTheDocument();
  });

  it("promotes a fallback link to primary via PATCH", async () => {
    renderModal([
      makeLink({ id: "a", provider: "bdtheque", is_primary: true }),
      makeLink({ id: "b", provider: "bdphile", is_primary: false }),
    ]);

    fireEvent.click(screen.getByRole("button", { name: "open" }));
    fireEvent.click(await screen.findByText("metadata.setPrimary"));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/metadata/links?id=b",
        expect.objectContaining({
          method: "PATCH",
          body: JSON.stringify({ is_primary: true, sync_series: true, sync_books: true }),
        }),
      ),
    );
  });

  it("shows the add-provider action in the linked view", async () => {
    renderModal([makeLink({ id: "a", provider: "bdtheque", is_primary: true })]);

    fireEvent.click(screen.getByRole("button", { name: "open" }));

    expect(await screen.findByText("metadata.addProvider")).toBeInTheDocument();
  });
});

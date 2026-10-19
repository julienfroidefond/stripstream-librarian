import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SeriesAddModal } from "@/app/(app)/discovery/components/SeriesAddModal";
import type { DiscoverySuggestion } from "@/app/(app)/discovery/page";
import type { SeriesCandidateDto } from "@/lib/api";

// Regression coverage for #112: adding a series from Discover then refreshing
// metadata used to fail with "Unknown provider: senscritique_bd" because the
// modal silently re-injected the discovery suggestion (and its provider) every
// time the user picked another provider.

const SUGGESTION: DiscoverySuggestion = {
  provider: "senscritique",
  external_id: "sc-1",
  title: "Blacksad",
  authors: ["Díaz Canales"],
  description: null,
  genres: ["Polar"],
  cover_url: null,
  external_url: null,
  start_year: 2000,
  total_volumes: 7,
  status: "FINISHED",
  rating: 8.5,
};

const BDPHILE_CANDIDATE: SeriesCandidateDto = {
  provider: "bdphile",
  external_id: "bdphile-1",
  title: "Blacksad Intégrale",
  authors: ["Díaz Canales"],
  description: null,
  publishers: [],
  start_year: 2000,
  total_volumes: 7,
  cover_url: null,
  external_url: null,
  confidence: 0.9,
  metadata_json: {},
};

const LIBRARIES = [{ id: "lib-1", name: "BD" }];

let fetchMock: ReturnType<typeof vi.fn>;

function jsonResponse(data: unknown, ok = true): Response {
  return { ok, status: ok ? 200 : 500, json: async () => data } as unknown as Response;
}

/** Body of the (first) POST to `url`, parsed from JSON. */
function postedBody(url: string): Record<string, unknown> {
  const call = fetchMock.mock.calls.find(([input]) => input === url);
  expect(call, `no fetch call for ${url}`).toBeTruthy();
  const init = call?.[1] as RequestInit | undefined;
  return JSON.parse((init?.body as string) ?? "{}");
}

beforeEach(() => {
  fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";

    if (url === "/api/metadata/providers") {
      return jsonResponse([
        { id: "senscritique", label: "SensCritique", requires_api_key: false },
        { id: "bdphile", label: "BDphile", requires_api_key: false },
      ]);
    }
    if (url.startsWith("/api/series?")) {
      return jsonResponse({ items: [] });
    }
    if (url === "/api/metadata/search" && method === "POST") {
      const body = JSON.parse((init?.body as string) ?? "{}");
      // The SensCritique query returns nothing: the modal is expected to inject
      // and pin the clicked discovery suggestion itself.
      return jsonResponse(body.provider === "bdphile" ? [BDPHILE_CANDIDATE] : []);
    }
    if (url === "/api/discovery/add-to-library") {
      return jsonResponse({ status: "ok" });
    }
    return jsonResponse([]);
  });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function renderModal() {
  render(
    <SeriesAddModal
      mode="discovery"
      suggestion={SUGGESTION}
      libraries={LIBRARIES}
      onAdded={vi.fn()}
    >
      {(open) => <button onClick={open}>open</button>}
    </SeriesAddModal>,
  );
}

/** Renders, opens the modal and waits for the initial (pinned) search to settle. */
async function openModal(): Promise<HTMLButtonElement> {
  renderModal();
  fireEvent.click(screen.getByRole("button", { name: "open" }));
  const addButton = await screen.findByRole("button", { name: "discovery.addToLibrary" });
  await waitFor(() => expect(addButton).toBeEnabled());
  return addButton as HTMLButtonElement;
}

describe("SeriesAddModal discovery provider handling", () => {
  it("adds the pinned discovery suggestion with its canonical provider", async () => {
    const addButton = await openModal();

    fireEvent.click(addButton);

    await waitFor(() => expect(postedBody("/api/discovery/add-to-library").provider).toBe("senscritique"));
  });

  it("uses the explicitly chosen provider instead of the suggestion", async () => {
    const addButton = await openModal();

    // Switch provider: the suggestion must not be re-injected/pinned.
    fireEvent.click(screen.getByRole("button", { name: "BDphile" }));

    // No candidate selected yet -> the fallback to the suggestion is off, so the
    // add button is disabled (this was the silent-fallback bug).
    await waitFor(() => expect(addButton).toBeDisabled());

    fireEvent.click(await screen.findByText("Blacksad Intégrale"));
    await waitFor(() => expect(addButton).toBeEnabled());

    fireEvent.click(addButton);

    await waitFor(() => expect(postedBody("/api/discovery/add-to-library").provider).toBe("bdphile"));
  });
});

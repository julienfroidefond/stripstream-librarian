import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DdlProviderSearchModal } from "@/app/components/DdlProviderSearchModal";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function ok(payload: unknown) {
  return { ok: true, status: 200, json: async () => payload, text: async () => "" };
}

const ENABLED_A = {
  id: "fourtoutici",
  label: "Fourtoutici",
  kind: "json",
  description: "desc",
  base_url: "https://fourtoutici.cc",
  requires_auth: false,
  enabled: true,
};
const ENABLED_B = { ...ENABLED_A, id: "annas", label: "Anna's Archive", enabled: true };

const RESULT = {
  provider: "fourtoutici",
  external_id: "file-1",
  filename: "BD Astérix le Gaulois 41 Astérix en Lusitanie.pdf",
  size: 20_000_000,
  series_name: "Astérix le Gaulois",
  volume_number: 41,
  all_volumes: [41],
  matched_missing_volumes: [41],
  match_confidence: "high",
  already_owned: false,
  download_id: null,
  download_status: null,
};

function routes(providers: unknown[], results: unknown[] = [RESULT]) {
  fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
    if (url === "/api/ddl-providers") return ok(providers);
    if (url.endsWith("/search")) return ok({ provider: "fourtoutici", query: "x", results });
    if (url === "/api/ddl-provider-downloads") return ok([]);
    if (url.includes("/downloads")) return ok({ id: "dl-1", status: "queued" });
    void init;
    return ok({});
  });
}

function renderModal(props: Partial<React.ComponentProps<typeof DdlProviderSearchModal>> = {}) {
  render(
    <DdlProviderSearchModal
      seriesName="Astérix"
      libraryId="lib-1"
      missingBooks={[{ title: null, volume_number: 41 }]}
      initialEnabled
      {...props}
    >
      {(open) => <button onClick={open}>open</button>}
    </DdlProviderSearchModal>
  );
}

describe("DdlProviderSearchModal", () => {
  it("renders nothing when no provider is enabled initially", () => {
    routes([{ ...ENABLED_A, enabled: false }]);
    renderModal({ initialEnabled: false });
    expect(screen.queryByText("open")).not.toBeInTheDocument();
  });

  it("renders nothing when the fetched list has no enabled provider", async () => {
    routes([{ ...ENABLED_A, enabled: false }]);
    renderModal();
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "open" })).not.toBeInTheDocument()
    );
    expect(fetchMock).toHaveBeenCalledWith("/api/ddl-providers");
  });

  it("lists enabled providers in a selector and searches on open", async () => {
    routes([ENABLED_A, ENABLED_B]);
    renderModal();

    fireEvent.click(await screen.findByRole("button", { name: "open" }));

    // Two enabled → selector; default provider is the first enabled.
    const selector = await screen.findByRole("combobox");
    expect(selector).toHaveValue("fourtoutici");

    // Default search runs against the series name.
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/ddl-providers/fourtoutici/search",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({
            query: "Astérix",
            series_name: "Astérix",
            library_id: "lib-1",
            missing_volumes: [41],
          }),
        })
      )
    );

    expect(await screen.findByText(RESULT.filename)).toBeInTheDocument();
    expect(screen.getByText(/ddlProviders\.matchedMissing/)).toBeInTheDocument();
  });

  it("queues a download against the browsed series, not the provider label", async () => {
    routes([ENABLED_A]);
    renderModal();

    fireEvent.click(await screen.findByRole("button", { name: "open" }));
    const downloadBtn = await screen.findByTitle("ddlProviders.download");
    fireEvent.click(downloadBtn);

    // `RESULT.series_name` is "Astérix le Gaulois": it must not override the target series.
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/ddl-providers/fourtoutici/downloads", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          external_id: "file-1",
          filename: RESULT.filename,
          size: RESULT.size,
          library_id: "lib-1",
          series_name: "Astérix",
          volume_number: 41,
        }),
      })
    );
    expect(await screen.findByText("ddlProviders.status.queued")).toBeInTheDocument();
  });

  it("re-searches with the volume query when a quick badge is used", async () => {
    routes([ENABLED_A], []);
    renderModal();

    fireEvent.click(await screen.findByRole("button", { name: "open" }));
    // The missing-book chip is labelled with its volume.
    fireEvent.click(await screen.findByRole("button", { name: "T41" }));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/ddl-providers/fourtoutici/search",
        expect.objectContaining({ body: expect.stringContaining("Astérix T41") })
      )
    );
  });
});

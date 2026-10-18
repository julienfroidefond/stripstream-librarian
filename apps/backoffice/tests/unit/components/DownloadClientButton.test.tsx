import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  DownloadClientBadge,
  DownloadClientButton,
  DownloadClientIndicator,
  DownloadClientProvider,
  DownloadClientSelector,
  isClientConfigured,
} from "@/app/components/DownloadClientButton";

afterEach(() => {
  vi.unstubAllGlobals();
  window.localStorage.clear();
});

describe("isClientConfigured", () => {
  it("requires a URL and a username for qBittorrent", () => {
    expect(isClientConfigured("qbittorrent", null)).toBe(false);
    expect(isClientConfigured("qbittorrent", { url: "http://qb" })).toBe(false);
    expect(isClientConfigured("qbittorrent", { url: "http://qb", username: " admin " })).toBe(true);
  });

  it("requires the enabled flag and a key for AllDebrid", () => {
    expect(isClientConfigured("alldebrid", { api_key: "k" })).toBe(false);
    expect(isClientConfigured("alldebrid", { enabled: true, api_key: "  " })).toBe(false);
    expect(isClientConfigured("alldebrid", { enabled: true, api_key: "k" })).toBe(true);
  });
});

describe("DownloadClientSelector", () => {
  it("stays hidden when fewer than two clients are available", () => {
    render(
      <DownloadClientProvider initialClients={{ qbittorrent: true, alldebrid: false }}>
        <DownloadClientSelector />
      </DownloadClientProvider>
    );
    expect(screen.queryByRole("group")).toBeNull();
  });

  it("switches the active client and persists the choice", () => {
    render(
      <DownloadClientProvider initialClients={{ qbittorrent: true, alldebrid: true }}>
        <DownloadClientSelector />
      </DownloadClientProvider>
    );

    const [qb, ad] = screen.getAllByRole("button");
    expect(qb).toHaveAttribute("aria-pressed", "true");
    expect(ad).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(ad);

    expect(ad).toHaveAttribute("aria-pressed", "true");
    expect(qb).toHaveAttribute("aria-pressed", "false");
    expect(window.localStorage.getItem("sl.downloadClient")).toBe("alldebrid");
  });
});

describe("DownloadClientIndicator", () => {
  it("shows the only configured client as a static label", () => {
    render(
      <DownloadClientProvider initialClients={{ qbittorrent: true, alldebrid: false }}>
        <DownloadClientIndicator />
      </DownloadClientProvider>
    );

    // The translation mock echoes the key and interpolates its variables.
    expect(screen.getByText(/downloadClient\.active/)).toBeInTheDocument();
    expect(screen.queryByRole("group")).toBeNull();
  });

  it("falls back to the segmented selector when several clients are available", () => {
    render(
      <DownloadClientProvider initialClients={{ qbittorrent: true, alldebrid: true }}>
        <DownloadClientIndicator />
      </DownloadClientProvider>
    );

    expect(screen.getByRole("group")).toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(2);
  });

  it("renders nothing when no client is configured", () => {
    render(
      <DownloadClientProvider initialClients={{ qbittorrent: false, alldebrid: false }}>
        <DownloadClientIndicator />
      </DownloadClientProvider>
    );

    expect(screen.queryByRole("group")).toBeNull();
    expect(screen.queryByText(/downloadClient\.active/)).toBeNull();
  });
});

describe("DownloadClientBadge", () => {
  it("renders the client brand name", () => {
    render(<DownloadClientBadge client="alldebrid" />);
    expect(screen.getByText("AllDebrid")).toBeInTheDocument();
  });
});

describe("DownloadClientButton", () => {
  it("renders nothing when no client is configured", () => {
    render(
      <DownloadClientProvider initialClients={{ qbittorrent: false, alldebrid: false }}>
        <DownloadClientButton downloadUrl="magnet:?x" releaseId="r1" />
      </DownloadClientProvider>
    );
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("snatches through the active client and notifies the parent", async () => {
    const onDownloadStarted = vi.fn();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ success: true, message: "ok" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(
      <DownloadClientProvider
        initialClients={{ qbittorrent: false, alldebrid: true }}
        onDownloadStarted={onDownloadStarted}
      >
        <DownloadClientButton
          downloadUrl="magnet:?x"
          releaseId="r1"
          libraryId="l1"
          seriesName="Série"
          expectedVolumes={[1, 2]}
        />
      </DownloadClientProvider>
    );

    fireEvent.click(screen.getByRole("button"));

    await waitFor(() => expect(onDownloadStarted).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/alldebrid/add",
      expect.objectContaining({ method: "POST" })
    );
    const body = JSON.parse(fetchMock.mock.calls[0][1].body as string);
    expect(body).toEqual({
      url: "magnet:?x",
      library_id: "l1",
      series_name: "Série",
      expected_volumes: [1, 2],
    });
  });

  it("surfaces the API error on the trigger", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ success: false, message: "bad magnet" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(
      <DownloadClientProvider initialClients={{ qbittorrent: true, alldebrid: false }}>
        <DownloadClientButton downloadUrl="magnet:?x" releaseId="r1" />
      </DownloadClientProvider>
    );

    fireEvent.click(screen.getByRole("button"));

    await waitFor(() =>
      expect(screen.getByRole("button")).toHaveAttribute("title", "bad magnet")
    );
  });
});

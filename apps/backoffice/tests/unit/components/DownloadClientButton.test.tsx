import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  DownloadClientBadge,
  DownloadClientButton,
  DownloadClientIndicator,
  DownloadClientProvider,
  isClientConfigured,
  readDownloadClientSetting,
} from "@/app/components/DownloadClientButton";

afterEach(() => {
  vi.unstubAllGlobals();
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

describe("readDownloadClientSetting", () => {
  it("keeps only known clients", () => {
    expect(readDownloadClientSetting(null)).toBeNull();
    expect(readDownloadClientSetting({})).toBeNull();
    expect(readDownloadClientSetting({ client: "sabnzbd" })).toBeNull();
    expect(readDownloadClientSetting({ client: "alldebrid" })).toBe("alldebrid");
    expect(readDownloadClientSetting({ client: "qbittorrent" })).toBe("qbittorrent");
  });
});

describe("DownloadClientProvider", () => {
  it("reads the client chosen in settings", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ client: "alldebrid" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(
      <DownloadClientProvider initialClients={{ qbittorrent: true, alldebrid: true }}>
        <DownloadClientIndicator />
      </DownloadClientProvider>
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/settings/download_client"));
    // qBittorrent comes first in the fallback order, the stored setting wins.
    await waitFor(() => expect(screen.getByText(/AllDebrid/)).toBeInTheDocument());
  });

  it("falls back to the first configured client when no choice is stored", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({}) });
    vi.stubGlobal("fetch", fetchMock);

    render(
      <DownloadClientProvider initialClients={{ qbittorrent: false, alldebrid: true }}>
        <DownloadClientButton downloadUrl="magnet:?x" releaseId="r1" />
      </DownloadClientProvider>
    );

    await waitFor(() => expect(screen.getByRole("button")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button"));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/alldebrid/add",
        expect.objectContaining({ method: "POST" })
      )
    );
  });
});

describe("DownloadClientIndicator", () => {
  it("shows the active client as a static label", () => {
    render(
      <DownloadClientProvider
        initialClients={{ qbittorrent: true, alldebrid: false }}
        initialClient={null}
      >
        <DownloadClientIndicator />
      </DownloadClientProvider>
    );

    // The translation mock echoes the key and interpolates its variables.
    expect(screen.getByText(/downloadClient\.active/)).toBeInTheDocument();
    expect(screen.getByText(/qBittorrent/)).toBeInTheDocument();
  });

  it("renders nothing when no client is configured", () => {
    render(
      <DownloadClientProvider
        initialClients={{ qbittorrent: false, alldebrid: false }}
        initialClient={null}
      >
        <DownloadClientIndicator />
      </DownloadClientProvider>
    );

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
      <DownloadClientProvider
        initialClients={{ qbittorrent: false, alldebrid: false }}
        initialClient={null}
      >
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
        initialClient={null}
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
      <DownloadClientProvider
        initialClients={{ qbittorrent: true, alldebrid: false }}
        initialClient={null}
      >
        <DownloadClientButton downloadUrl="magnet:?x" releaseId="r1" />
      </DownloadClientProvider>
    );

    fireEvent.click(screen.getByRole("button"));

    await waitFor(() =>
      expect(screen.getByRole("button")).toHaveAttribute("title", "bad magnet")
    );
  });
});

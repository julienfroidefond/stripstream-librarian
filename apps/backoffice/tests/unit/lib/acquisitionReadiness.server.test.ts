import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchSettingsSnapshot: vi.fn(),
  getTelegramMonitorStatus: vi.fn(),
  fetchDdlProviders: vi.fn(),
}));

vi.mock("@/lib/api", () => api);

import { loadAcquisitionReadinessUncached } from "@/lib/acquisitionReadiness.server";

const readySettings = {
  downloads_enabled: { enabled: true },
  prowlarr: { enabled: true, url: "http://p", api_key: "key" },
  qbittorrent: { enabled: true, url: "http://q", username: "admin" },
  alldebrid: { enabled: true, api_key: "key" },
};

beforeEach(() => {
  api.fetchSettingsSnapshot.mockReset().mockResolvedValue(readySettings);
  api.getTelegramMonitorStatus
    .mockReset()
    .mockResolvedValue({ configured: true, authorized: true });
  api.fetchDdlProviders.mockReset().mockResolvedValue([{ enabled: true }]);
});

describe("loadAcquisitionReadinessUncached", () => {
  it("reads the three sources once and resolves a ready snapshot", async () => {
    await expect(loadAcquisitionReadinessUncached()).resolves.toEqual({
      downloads: true,
      prowlarr: true,
      qbittorrent: true,
      alldebrid: true,
      telegram: true,
      ddl: true,
    });

    expect(api.fetchSettingsSnapshot).toHaveBeenCalledTimes(1);
    expect(api.getTelegramMonitorStatus).toHaveBeenCalledWith({ revalidate: 60 });
    expect(api.fetchDdlProviders).toHaveBeenCalledWith({ revalidate: 60 });
  });

  it("returns an all-off readiness when the settings snapshot fails", async () => {
    api.fetchSettingsSnapshot.mockRejectedValue(new Error("offline"));

    await expect(loadAcquisitionReadinessUncached()).resolves.toEqual({
      downloads: false,
      prowlarr: false,
      qbittorrent: false,
      alldebrid: false,
      telegram: false,
      ddl: false,
    });
  });

  it("keeps Prowlarr and clients available when Telegram and DDL fail", async () => {
    api.getTelegramMonitorStatus.mockRejectedValue(new Error("telegram down"));
    api.fetchDdlProviders.mockRejectedValue(new Error("ddl down"));

    await expect(loadAcquisitionReadinessUncached()).resolves.toEqual({
      downloads: true,
      prowlarr: true,
      qbittorrent: true,
      alldebrid: true,
      telegram: false,
      ddl: false,
    });
  });

  it("honours the master flag without exposing the other sources", async () => {
    api.fetchSettingsSnapshot.mockResolvedValue({
      ...readySettings,
      downloads_enabled: { enabled: false },
    });

    await expect(loadAcquisitionReadinessUncached()).resolves.toEqual({
      downloads: false,
      prowlarr: false,
      qbittorrent: false,
      alldebrid: false,
      telegram: false,
      ddl: false,
    });
  });
});

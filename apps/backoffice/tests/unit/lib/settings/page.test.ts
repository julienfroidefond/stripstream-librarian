// @vitest-environment node

import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchApiVersion: vi.fn(),
  fetchIndexerVersion: vi.fn(),
  fetchLibraries: vi.fn(),
  fetchProviderStatuses: vi.fn(),
  fetchSeriesStatuses: vi.fn(),
  fetchStatusMappings: vi.fn(),
  fetchUsers: vi.fn(),
  getCacheStats: vi.fn(),
  getSettings: vi.fn(),
  getThumbnailStats: vi.fn(),
}));

vi.mock("@/lib/api", () => api);

import packageJson from "@/package.json";
import { loadSettingsPage, type SettingsSectionName } from "@/lib/settings/page";

const settings = { limits: { concurrent_renders: 6 }, downloads_enabled: { enabled: true } };
const cacheStats = {
  total_size_mb: 12.5,
  file_count: 40,
  directory: "/srv/cache",
  memory_size_mb: 3,
  memory_page_count: 9,
  memory_max_size_mb: 256,
};
const thumbnailStats = { total_size_mb: 4, file_count: 10, directory: "/srv/thumbs" };
const users = [{ id: "u1", username: "reader" }];
const statusMappings = [{ id: "m1", provider_status: "Ongoing", mapped_status: "ongoing" }];
const libraries = [{ id: "lib-1", name: "Main" }];

/** Every section of the loader and the stub it reads. */
const sections: Array<[SettingsSectionName, keyof typeof api]> = [
  ["settings", "getSettings"],
  ["cacheStats", "getCacheStats"],
  ["thumbnailStats", "getThumbnailStats"],
  ["users", "fetchUsers"],
  ["statusMappings", "fetchStatusMappings"],
  ["seriesStatuses", "fetchSeriesStatuses"],
  ["providerStatuses", "fetchProviderStatuses"],
  ["libraries", "fetchLibraries"],
  ["apiVersion", "fetchApiVersion"],
  ["indexerVersion", "fetchIndexerVersion"],
];

beforeEach(() => {
  api.getSettings.mockResolvedValue(settings);
  api.getCacheStats.mockResolvedValue(cacheStats);
  api.getThumbnailStats.mockResolvedValue(thumbnailStats);
  api.fetchUsers.mockResolvedValue(users);
  api.fetchStatusMappings.mockResolvedValue(statusMappings);
  api.fetchSeriesStatuses.mockResolvedValue(["ongoing", "ended"]);
  api.fetchProviderStatuses.mockResolvedValue(["Ongoing", "Completed"]);
  api.fetchLibraries.mockResolvedValue(libraries);
  api.fetchApiVersion.mockResolvedValue("3.32.0");
  api.fetchIndexerVersion.mockResolvedValue("3.31.0");
});

describe("loadSettingsPage", () => {
  it("resolves every section into one model when the API answers", async () => {
    const model = await loadSettingsPage();

    expect(model).toEqual({
      settings,
      cacheStats,
      thumbnailStats,
      users,
      statusMappings,
      seriesStatuses: ["ongoing", "ended"],
      providerStatuses: ["Ongoing", "Completed"],
      libraries,
      versions: { api: "3.32.0", indexer: "3.31.0", backoffice: packageJson.version },
      degraded: [],
    });
  });

  it.each(sections)("degrades %s alone when its fetch fails", async (section, stub) => {
    api[stub].mockRejectedValue(new Error("down"));

    const model = await loadSettingsPage();

    expect(model.degraded).toEqual([section]);
  });

  it("returns null, never made-up values, for failed stats, versions and settings", async () => {
    for (const [, stub] of sections) api[stub].mockRejectedValue(new Error("down"));

    const model = await loadSettingsPage();

    expect(model).toEqual({
      settings: null,
      cacheStats: null,
      thumbnailStats: null,
      users: [],
      statusMappings: [],
      seriesStatuses: [],
      providerStatuses: [],
      libraries: [],
      versions: { api: null, indexer: null, backoffice: packageJson.version },
      degraded: sections.map(([section]) => section),
    });
  });

  it("marks the settings unread instead of seeding an empty map as acknowledged", async () => {
    api.getSettings.mockRejectedValue(new Error("down"));

    const model = await loadSettingsPage();

    // `null` (not `{}`) is what switches the settings draft to read-only.
    expect(model.settings).toBeNull();
    expect(model.degraded).toContain("settings");
  });
});

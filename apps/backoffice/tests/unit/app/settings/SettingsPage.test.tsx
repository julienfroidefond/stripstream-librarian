import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import SettingsPage from "@/app/(app)/settings/SettingsPage";
import type { SettingsPageModel } from "@/lib/settings/page";

function model(overrides: Partial<SettingsPageModel> = {}): SettingsPageModel {
  return {
    settings: { limits: { concurrent_renders: 4, timeout_seconds: 12, rate_limit_per_second: 120 } },
    cacheStats: {
      total_size_mb: 1,
      file_count: 2,
      directory: "/srv/cache",
      memory_size_mb: 0,
      memory_page_count: 0,
      memory_max_size_mb: 128,
    },
    thumbnailStats: { total_size_mb: 1, file_count: 2, directory: "/srv/thumbs" },
    users: [],
    statusMappings: [],
    seriesStatuses: [],
    providerStatuses: [],
    libraries: [],
    versions: { api: "1.0.0", indexer: "1.0.0", backoffice: "1.0.0" },
    degraded: [],
    ...overrides,
  };
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  // The proxy acknowledges a section write by echoing the stored value.
  fetchMock = vi.fn(async (_url: RequestInfo | URL, init?: RequestInit) =>
    new Response(init?.body ? JSON.stringify(JSON.parse(String(init.body)).value) : "{}", {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }),
  );
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const flush = () => act(() => new Promise((resolve) => setTimeout(resolve, 0)));

function settingsWrites() {
  return fetchMock.mock.calls.filter(
    ([url, init]) => String(url).startsWith("/api/settings/") && init?.method === "POST",
  );
}

async function editConcurrentRenders(value: string) {
  const [input] = screen.getAllByRole("spinbutton");
  fireEvent.change(input, { target: { value } });
  fireEvent.blur(input);
  await flush();
  return input;
}

describe("SettingsPage", () => {
  it("saves an edited section when the stored settings were loaded", async () => {
    render(<SettingsPage model={model()} initialTab="general" />);

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await editConcurrentRenders("8");

    expect(settingsWrites()).toHaveLength(1);
    expect(String(settingsWrites()[0][0])).toBe("/api/settings/limits");
  });

  it("is read-only when the settings section is degraded: banner, and no save is possible", async () => {
    render(
      <SettingsPage model={model({ settings: null, degraded: ["settings"] })} initialTab="general" />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("settings.readOnlyTitle");
    const input = await editConcurrentRenders("8");

    // The edit is refused (the field keeps the default) and nothing is sent:
    // defaults must never overwrite the stored configuration.
    expect(input).toHaveValue(4);
    expect(settingsWrites()).toHaveLength(0);
  });

  it("shows unavailable stats instead of made-up directories", async () => {
    fetchMock.mockImplementation(async () => new Response("down", { status: 503 }));
    render(
      <SettingsPage
        model={model({ cacheStats: null, thumbnailStats: null, degraded: ["cacheStats", "thumbnailStats"] })}
        initialTab="media"
      />,
    );
    await flush();

    expect(screen.getAllByText("settings.statsUnavailable")).toHaveLength(2);
    expect(screen.queryByText("/tmp/stripstream-image-cache")).not.toBeInTheDocument();
    expect(screen.queryByText("/data/thumbnails")).not.toBeInTheDocument();
  });

  it("shows an unknown version as ?", () => {
    render(
      <SettingsPage
        model={model({ versions: { api: null, indexer: null, backoffice: "9.9.9" }, degraded: ["apiVersion", "indexerVersion"] })}
        initialTab="general"
      />,
    );

    expect(screen.getAllByText("?")).toHaveLength(2);
    expect(screen.getByText("9.9.9")).toBeInTheDocument();
  });
});

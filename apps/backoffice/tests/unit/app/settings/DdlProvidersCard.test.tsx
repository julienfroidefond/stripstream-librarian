import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/app/components/ui")>();
  return { ...actual, toast: vi.fn() };
});

import { toast } from "@/app/components/ui";

import { DdlProvidersCard } from "@/app/(app)/settings/components/DdlProvidersCard";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  vi.mocked(toast).mockReset();
});

function ok(payload: unknown) {
  return { ok: true, status: 200, json: async () => payload, text: async () => "" };
}

const PROVIDERS = [
  {
    id: "fourtoutici",
    label: "Fourtoutici",
    kind: "json",
    description: "Catalogue BD / manga.",
    base_url: "https://fourtoutici.cc",
    requires_auth: false,
    enabled: false,
  },
  {
    id: "annas",
    label: "Anna's Archive",
    kind: "html",
    description: "Bibliothèque de secours.",
    base_url: "https://annas-archive.cc",
    requires_auth: false,
    enabled: true,
  },
];

function routes(overrides: Record<string, (init?: RequestInit) => unknown> = {}) {
  fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    const key = `${method} ${url}`;
    if (key in overrides) return overrides[key](init);
    if (url === "/api/ddl-providers") return ok(PROVIDERS);
    if (url === "/api/ddl-providers/search-settings") return ok({ search_interval_minutes: 0 });
    return ok({});
  });
}

describe("DdlProvidersCard", () => {
  it("renders a row per provider with its enabled state", async () => {
    routes();
    render(<DdlProvidersCard />);

    expect(await screen.findByText("Fourtoutici")).toBeInTheDocument();
    expect(screen.getByText("Anna's Archive")).toBeInTheDocument();
    expect(screen.getByText("ddlProviders.kindJson")).toBeInTheDocument();
    expect(screen.getByText("ddlProviders.kindHtml")).toBeInTheDocument();
    expect(screen.getByLabelText("Fourtoutici")).not.toBeChecked();
    expect(screen.getByLabelText("Anna's Archive")).toBeChecked();
  });

  it("enables a provider and sends the current base URL", async () => {
    routes({
      "POST /api/ddl-providers/fourtoutici/toggle": () =>
        ok([{ ...PROVIDERS[0], enabled: true }, PROVIDERS[1]]),
    });
    render(<DdlProvidersCard />);

    const toggle = await screen.findByLabelText("Fourtoutici");
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/ddl-providers/fourtoutici/toggle", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ enabled: true, base_url: "https://fourtoutici.cc" }),
      })
    );
    expect(toast).toHaveBeenCalledWith("ddlProviders.saved", "success");
  });

  it("persists a base URL override on blur", async () => {
    routes({
      "POST /api/ddl-providers/fourtoutici/toggle": () =>
        ok([{ ...PROVIDERS[0], base_url: "https://mirror.example" }, PROVIDERS[1]]),
    });
    render(<DdlProvidersCard />);

    const input = await screen.findByDisplayValue("https://fourtoutici.cc");
    fireEvent.change(input, { target: { value: "https://mirror.example" } });
    fireEvent.blur(input);

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/ddl-providers/fourtoutici/toggle", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ enabled: false, base_url: "https://mirror.example" }),
      })
    );
  });

  it("probes a provider and shows the result message", async () => {
    routes({
      "POST /api/ddl-providers/fourtoutici/test": () =>
        ok({ success: true, message: "12 résultats", result_count: 12 }),
    });
    render(<DdlProvidersCard />);

    const buttons = await screen.findAllByRole("button", { name: /ddlProviders.test$/ });
    fireEvent.click(buttons[0]);

    expect(await screen.findByText("12 résultats")).toBeInTheDocument();
  });

  it("reports a load failure", async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 500, json: async () => ({}), text: async () => "" });
    render(<DdlProvidersCard />);

    expect(await screen.findByText("ddlProviders.loadError")).toBeInTheDocument();
  });

  it("saves a new background-search cadence", async () => {
    routes({
      "POST /api/ddl-providers/search-settings": (init) => {
        const body = JSON.parse(String(init?.body)) as { search_interval_minutes: number };
        return ok({ search_interval_minutes: body.search_interval_minutes });
      },
    });
    render(<DdlProvidersCard />);

    const select = await screen.findByLabelText("ddlProviders.searchInterval");
    fireEvent.change(select, { target: { value: "60" } });

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/ddl-providers/search-settings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ search_interval_minutes: 60 }),
      })
    );
  });

  it("starts a global search on demand", async () => {
    routes({
      "POST /api/ddl-providers/search-all": () => ok({ id: "job-1", status: "pending" }),
    });
    render(<DdlProvidersCard />);

    const button = await screen.findByRole("button", { name: /ddlProviders.searchNow/ });
    fireEvent.click(button);

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/ddl-providers/search-all", { method: "POST" })
    );
    await waitFor(() =>
      expect(toast).toHaveBeenCalledWith("ddlProviders.searchQueued", "success")
    );
  });

  it("disables the on-demand search when no provider is enabled", async () => {
    routes({
      "GET /api/ddl-providers": () => ok(PROVIDERS.map((p) => ({ ...p, enabled: false }))),
    });
    render(<DdlProvidersCard />);

    const button = await screen.findByRole("button", { name: /ddlProviders.searchNow/ });
    expect(button).toBeDisabled();
  });
});

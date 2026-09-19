import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { KomgaSyncCard } from "@/app/(app)/settings/components/KomgaSyncCard";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function ok(payload: unknown) {
  return { ok: true, json: async () => payload };
}

function installRoutes(overrides: Record<string, unknown> = {}) {
  fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    const key = `${method} ${url}`;
    if (key in overrides) {
      const value = overrides[key];
      return typeof value === "function" ? (value as () => unknown)() : value;
    }
    if (url === "/api/komga/reports") return ok([]);
    if (url.startsWith("/api/komga/reports/")) return ok({});
    return ok({});
  });
}

const reportSummary = {
  id: "r1",
  created_at: "2024-01-02T10:00:00Z",
  komga_url: "http://komga",
  total_komga_read: 10,
  matched: 2,
  newly_marked: 1,
  unmatched_count: 1,
};

const syncResponse = {
  total_komga_read: 12,
  matched: 3,
  already_read: 2,
  newly_marked: 1,
  matched_books: ["A", "B"],
  unmatched: ["X"],
};

describe("KomgaSyncCard", () => {
  it("loads reports and syncs, revealing matched and unmatched books", async () => {
    installRoutes({
      "GET /api/komga/reports": () => ok([reportSummary]),
      "POST /api/komga/sync": () => ok(syncResponse),
    });
    render(
      <KomgaSyncCard
        users={[
          { id: "9", username: "Alice" } as never,
          { id: "10", username: "Carol" } as never,
        ]}
        initialData={{ url: "http://komga", username: "bob", user_id: "9" }}
      />,
    );

    await waitFor(() => expect(screen.getByText("settings.syncHistory")).toBeInTheDocument());
    expect(screen.getByText("http://komga")).toBeInTheDocument();

    fireEvent.change(screen.getByDisplayValue("bob"), { target: { value: "bobby" } });
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "10" } });
    fireEvent.change(document.querySelector('input[type="password"]') as HTMLInputElement, {
      target: { value: "secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.syncReadBooks" }));

    await waitFor(() => expect(screen.getByText("12")).toBeInTheDocument());
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/settings/komga",
      expect.objectContaining({ method: "POST" }),
    );

    fireEvent.click(screen.getByText(/settings\.matchedBooks/));
    expect(screen.getByText("A")).toBeInTheDocument();
    fireEvent.click(screen.getByText(/settings\.unmatchedBooks/));
    expect(screen.getByText("X")).toBeInTheDocument();
  });

  it("falls back to the first user and reports sync errors", async () => {
    installRoutes({
      "POST /api/komga/sync": () => ({ ok: false, json: async () => ({ error: "bad credentials" }) }),
    });
    render(
      <KomgaSyncCard
        users={[{ id: "1", username: "First" } as never]}
        initialData={{ username: "first", user_id: "1" }}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText("https://komga.example.com"), {
      target: { value: "http://k" },
    });
    fireEvent.change(document.querySelector('input[type="password"]') as HTMLInputElement, {
      target: { value: "secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.syncReadBooks" }));

    await waitFor(() => expect(screen.getByText("bad credentials")).toBeInTheDocument());
  });

  it("shows a connection error when the sync request throws", async () => {
    installRoutes({
      "POST /api/komga/sync": () => {
        throw new Error("network");
      },
    });
    render(
      <KomgaSyncCard
        users={[] as never}
        initialData={{ username: "bob", user_id: "1" }}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText("https://komga.example.com"), {
      target: { value: "http://k" },
    });
    fireEvent.change(document.querySelector('input[type="password"]') as HTMLInputElement, {
      target: { value: "secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.syncReadBooks" }));

    await waitFor(() =>
      expect(screen.getByText("Failed to connect to sync endpoint")).toBeInTheDocument(),
    );
  });

  it("opens a stored report and toggles its sections", async () => {
    installRoutes({
      "GET /api/komga/reports": () => ok([reportSummary]),
      "GET /api/komga/reports/r1": () =>
        ok({
          id: "r1",
          total_komga_read: 5,
          matched: 2,
          already_read: 1,
          newly_marked: 1,
          matched_books: ["M1"],
          unmatched: ["U1"],
        }),
    });
    render(<KomgaSyncCard users={[] as never} initialData={null} />);

    await waitFor(() => expect(screen.getByText("http://komga")).toBeInTheDocument());
    fireEvent.click(screen.getByText("http://komga"));

    await waitFor(() => expect(screen.getByText(/settings\.matchedBooks/)).toBeInTheDocument());
    fireEvent.click(screen.getByText(/settings\.matchedBooks/));
    expect(screen.getByText("M1")).toBeInTheDocument();
    fireEvent.click(screen.getByText(/settings\.unmatchedBooks/));
    expect(screen.getByText("U1")).toBeInTheDocument();
  });

  it("ignores report loading failures", async () => {
    installRoutes({
      "GET /api/komga/reports": () => {
        throw new Error("down");
      },
    });
    render(<KomgaSyncCard users={[] as never} initialData={null} />);
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(screen.queryByText("settings.syncHistory")).not.toBeInTheDocument();
  });
});

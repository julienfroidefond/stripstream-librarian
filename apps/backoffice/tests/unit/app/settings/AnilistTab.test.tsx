import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AnilistTab } from "@/app/(app)/settings/components/AnilistTab";

const fetchMock = vi.fn();
const locationMock = { origin: "http://localhost:7082", href: "" };

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  locationMock.href = "";
  vi.stubGlobal("location", locationMock);
});

function ok(payload: unknown) {
  return { ok: true, json: async () => payload };
}

describe("AnilistTab", () => {
  it("renders saved settings, saves, connects and runs reports", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
      if (url === "/api/anilist/status") {
        return ok({ username: "viewer", site_url: "http://anilist/u", user_id: 5 });
      }
      if (url === "/api/anilist/sync/preview") {
        return ok([
          { anilist_id: 1, anilist_title: "Same", series_name: "Same", books_read: 1, book_count: 2, status: "COMPLETED" },
          { anilist_id: 2, series_name: "Other", books_read: 0, book_count: 3, status: "CURRENT" },
        ]);
      }
      if (url === "/api/anilist/sync" && init?.method === "POST") {
        return ok({
          synced: 2,
          skipped: 1,
          errors: ["e1"],
          items: [
            { series_name: "S1", anilist_url: "http://a/1", status: "COMPLETED", progress_volumes: 2 },
            { series_name: "S2", anilist_title: "Alt", status: "PAUSED", progress_volumes: 0 },
          ],
        });
      }
      if (url === "/api/anilist/pull" && init?.method === "POST") {
        return ok({
          updated: 1,
          skipped: 1,
          errors: ["p1"],
          items: [
            { series_name: "P1", anilist_status: "COMPLETED", books_updated: 3 },
            { series_name: "P2", anilist_status: "CURRENT", books_updated: 1 },
            { series_name: "P3", anilist_status: "PLANNING", books_updated: 0 },
            { series_name: "P4", anilist_status: "DROPPED", books_updated: 0 },
          ],
        });
      }
      return ok({});
    });

    render(
      <AnilistTab
        handleUpdateSetting={handleUpdateSetting}
        users={[
          { id: "u1", username: "Local" } as never,
          { id: "u2", username: "Second" } as never,
        ]}
        initialData={{ client_id: "cid", access_token: "tok", user_id: "5", local_user_id: "u1" }}
      />,
    );

    expect(await screen.findByText("http://localhost:7082/anilist/callback")).toBeInTheDocument();

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "u2" } });
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "anilist",
      expect.objectContaining({ local_user_id: "u2" }),
    );

    await userEvent.click(screen.getByRole("button", { name: "common.save" }));
    expect(handleUpdateSetting).toHaveBeenCalledWith("anilist", expect.objectContaining({ client_id: "cid" }));

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistConnectButton" }));
    await waitFor(() => expect(locationMock.href).toContain("client_id=cid"));

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistTestConnection" }));
    expect(await screen.findByText(/settings\.anilistConnected/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistPreviewButton" }));
    expect(await screen.findByText("Same")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "✕" }));
    expect(screen.queryByText("Same")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistSyncButton" }));
    expect(await screen.findByText("S1")).toBeInTheDocument();
    expect(screen.getByText("Alt")).toBeInTheDocument();
    expect(screen.getByText("e1")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistPullButton" }));
    expect(await screen.findByText("P1")).toBeInTheDocument();
    expect(screen.getByText("p1")).toBeInTheDocument();
  });

  it("handles empty initial data and surfaces action errors", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockImplementation(async (url: string) => {
      if (url === "/api/anilist/status") {
        return { ok: false, json: async () => ({ error: "bad token" }) };
      }
      if (url === "/api/anilist/sync/preview") return ok([]);
      if (url === "/api/anilist/sync") return { ok: false, json: async () => ({}) };
      if (url === "/api/anilist/pull") throw "down";
      return ok({});
    });

    render(<AnilistTab handleUpdateSetting={handleUpdateSetting} users={[] as never} initialData={null} />);

    expect(await screen.findByText("http://localhost:7082/anilist/callback")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("settings.anilistClientIdPlaceholder"), {
      target: { value: "cid2" },
    });
    fireEvent.change(screen.getByPlaceholderText("settings.anilistTokenPlaceholder"), {
      target: { value: "tok2" },
    });
    fireEvent.change(screen.getByPlaceholderText("settings.anilistUserIdPlaceholder"), {
      target: { value: "9" },
    });

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistTestConnection" }));
    expect(await screen.findByText("bad token")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistPreviewButton" }));
    expect(await screen.findByText("settings.anilistPreviewEmpty")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistSyncButton" }));
    expect(await screen.findByText("Sync failed")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistPullButton" }));
    expect(await screen.findByText("Pull failed")).toBeInTheDocument();
  });

  it("reports a connection failure and auto-fills the user id", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockImplementation(async (url: string) => {
      if (url === "/api/anilist/status") return ok({ username: "v", site_url: "http://u", user_id: 77 });
      return ok({});
    });

    render(
      <AnilistTab
        handleUpdateSetting={handleUpdateSetting}
        users={[] as never}
        initialData={{ access_token: "tok" }}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistTestConnection" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/anilist/status"));
    expect(screen.getByDisplayValue("77")).toBeInTheDocument();
  });

  it("surfaces a thrown connection error", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockRejectedValue("boom");

    render(
      <AnilistTab
        handleUpdateSetting={handleUpdateSetting}
        users={[] as never}
        initialData={{ access_token: "tok" }}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistTestConnection" }));
    expect(await screen.findByText("Connection failed")).toBeInTheDocument();
  });

  it("surfaces a non-error preview failure", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockImplementation(async (url: string) => {
      if (url === "/api/anilist/sync/preview") throw "down";
      return ok({});
    });

    render(<AnilistTab handleUpdateSetting={handleUpdateSetting} users={[] as never} initialData={null} />);

    await userEvent.click(screen.getByRole("button", { name: "settings.anilistPreviewButton" }));
    expect(await screen.findByText("Preview failed")).toBeInTheDocument();
  });
});

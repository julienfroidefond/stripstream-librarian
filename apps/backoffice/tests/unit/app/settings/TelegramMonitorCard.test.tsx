import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/app/components/ui")>();
  return { ...actual, toast: vi.fn() };
});

import { toast } from "@/app/components/ui";

import { TelegramMonitorCard } from "@/app/(app)/settings/components/TelegramMonitorCard";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  vi.mocked(toast).mockReset();
});

function ok(payload: unknown) {
  return { ok: true, json: async () => payload, text: async () => "" };
}

const statusBase = {
  authorized: false,
  configured: true,
  phone: "+331",
  api_id: 123,
  sync_interval_minutes: 60,
};

function routes(overrides: Record<string, (init?: RequestInit) => unknown> = {}) {
  fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    const key = `${method} ${url}`;
    if (key in overrides) return overrides[key](init);
    if (url === "/api/telegram-monitor/status") return ok(statusBase);
    if (url === "/api/telegram-monitor/sources") return ok([]);
    if (url.startsWith("/api/telegram-monitor/channels")) return ok([]);
    return ok({});
  });
}

describe("TelegramMonitorCard", () => {
  it("saves settings, sends the code and verifies authorization", async () => {
    let authorized = false;
    routes({
      "GET /api/telegram-monitor/status": () => ok({ ...statusBase, authorized }),
      "POST /api/telegram-monitor/settings": () => ok({}),
      "POST /api/telegram-monitor/auth/start": () => ok({}),
      "POST /api/telegram-monitor/auth/verify": () => {
        authorized = true;
        return ok({});
      },
    });

    render(
      <TelegramMonitorCard
        initialLibraries={[{ id: "l1", name: "Lib1" } as never]}
        initialConcurrentDownloads={3}
      />,
    );

    expect(await screen.findByDisplayValue("123")).toBeInTheDocument();
    expect(screen.getByDisplayValue("+331")).toBeInTheDocument();

    fireEvent.change(screen.getByDisplayValue("123"), { target: { value: "456" } });
    fireEvent.change(screen.getByDisplayValue("+331"), { target: { value: "+332" } });

    const hash = screen.getByPlaceholderText("telegramMonitor.apiHashPlaceholder");
    fireEvent.change(hash, { target: { value: "hash" } });
    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.saveSettings" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("settings.savedSuccess", "success"));

    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.sendCode" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("telegramMonitor.codeSent", "success"));

    const code = screen.getByPlaceholderText("telegramMonitor.codePlaceholder");
    fireEvent.keyDown(code, { key: "Enter" });
    fireEvent.change(code, { target: { value: "12345" } });
    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.verify" }));

    expect(await screen.findByText("telegramMonitor.sourcesTitle")).toBeInTheDocument();
    expect(screen.getByText("telegramMonitor.noSources")).toBeInTheDocument();

    const interval = screen.getAllByRole("combobox")[0];
    expect((interval as HTMLSelectElement).value).toBe("60");
    fireEvent.change(interval, { target: { value: "1440" } });
    await waitFor(() => expect(toast).toHaveBeenCalledWith("settings.savedSuccess", "success"));

    fireEvent.change(screen.getAllByRole("combobox")[1], { target: { value: "l1" } });

    const concurrent = screen.getByDisplayValue("3");
    fireEvent.change(concurrent, { target: { value: "5" } });
    fireEvent.blur(concurrent);
  });

  it("manages sources and channel suggestions", async () => {
    routes({
      "GET /api/telegram-monitor/status": () => ok({ ...statusBase, authorized: true }),
      "GET /api/telegram-monitor/sources": () =>
        ok([{ id: "s1", channel_username: "c1", channel_title: "Title1" }]),
      "GET /api/telegram-monitor/channels?q=cha": () =>
        ok([
          { username: "chan", title: "Chan", kind: "channel" },
          { username: null, title: "Group", kind: "group" },
        ]),
      "POST /api/telegram-monitor/sources": () =>
        ok({ id: "s2", channel_username: "newchan", channel_title: null }),
      "DELETE /api/telegram-monitor/sources/s1": () => ok({}),
      "DELETE /api/telegram-monitor/auth": () => ok({}),
    });

    render(
      <TelegramMonitorCard
        initialLibraries={[{ id: "l1", name: "Lib1" } as never]}
        onSaveConcurrentDownloads={vi.fn()}
      />,
    );

    expect(await screen.findByText("@c1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "telegramMonitor.add" }));

    const search = screen.getByPlaceholderText("telegramMonitor.channelSearchPlaceholder");
    fireEvent.focus(search);
    fireEvent.change(search, { target: { value: "cha" } });

    const suggestion = await screen.findByText("Chan");
    fireEvent.mouseDown(suggestion);

    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.add" }));
    expect(await screen.findByText("@newchan")).toBeInTheDocument();

    await userEvent.click(screen.getAllByText("×")[0]);
    await waitFor(() => expect(screen.queryByText("@c1")).not.toBeInTheDocument());

    fireEvent.change(search, { target: { value: " " } });
    fireEvent.mouseDown(document.body);

    fireEvent.click(screen.getByRole("button", { name: /telegramMonitor.apiCredentials/ }));
    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.disconnect" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      "/api/telegram-monitor/auth",
      expect.objectContaining({ method: "DELETE" }),
    ));
  });

  it("reports request failures", async () => {
    const failing = { ok: false, json: async () => ({}), text: async () => "boom" };
    routes({
      "GET /api/telegram-monitor/status": () => ok({ ...statusBase, authorized: true }),
      "GET /api/telegram-monitor/sources": () =>
        ok([{ id: "s1", channel_username: "c1", channel_title: "T1" }]),
      "POST /api/telegram-monitor/settings": () => failing,
      "POST /api/telegram-monitor/sources": () => failing,
      "DELETE /api/telegram-monitor/sources/s1": () => failing,
      "DELETE /api/telegram-monitor/auth": () => failing,
      "GET /api/telegram-monitor/channels?q=x": () => failing,
    });

    render(<TelegramMonitorCard />);
    expect(await screen.findByText("@c1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /telegramMonitor.apiCredentials/ }));
    const hash = await screen.findByPlaceholderText("telegramMonitor.apiHashPlaceholder");
    fireEvent.change(hash, { target: { value: "hash" } });
    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.saveSettings" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("settings.savedError", "error"));

    fireEvent.change(screen.getByPlaceholderText("telegramMonitor.channelSearchPlaceholder"), {
      target: { value: "x" },
    });
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/telegram-monitor/channels?q=x",
        expect.objectContaining({}),
      ),
    );

    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.add" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Error: boom", "error"));

    await userEvent.click(screen.getAllByText("×")[0]);
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Error: boom", "error"));

    fireEvent.change(screen.getAllByRole("combobox")[0], { target: { value: "1440" } });
    await waitFor(() => expect(toast).toHaveBeenCalledWith("settings.savedError", "error"));

    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.disconnect" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "telegramMonitor.disconnect" })).toBeInTheDocument());
  });

  it("reports auth failures and guards empty inputs", async () => {
    routes({
      "GET /api/telegram-monitor/status": () => ok(statusBase),
      "POST /api/telegram-monitor/auth/start": () => ({
        ok: false,
        json: async () => ({}),
        text: async () => "nope",
      }),
    });

    render(<TelegramMonitorCard />);
    expect(await screen.findByRole("button", { name: "telegramMonitor.sendCode" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "telegramMonitor.saveSettings" }));

    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.sendCode" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Error: nope", "error"));

    const header = screen.getByRole("button", { name: /telegramMonitor.apiCredentials/ });
    fireEvent.click(header);
    fireEvent.click(header);
    fireEvent.click(header);
  });

  it("reports verify failures", async () => {
    routes({
      "GET /api/telegram-monitor/status": () => ok(statusBase),
      "POST /api/telegram-monitor/auth/start": () => ok({}),
      "POST /api/telegram-monitor/auth/verify": () => ({
        ok: false,
        json: async () => ({}),
        text: async () => "bad code",
      }),
    });

    render(<TelegramMonitorCard />);
    await userEvent.click(await screen.findByRole("button", { name: "telegramMonitor.sendCode" }));

    const code = await screen.findByPlaceholderText("telegramMonitor.codePlaceholder");
    fireEvent.change(code, { target: { value: "0000" } });
    await userEvent.click(screen.getByRole("button", { name: "telegramMonitor.verify" }));

    await waitFor(() => expect(toast).toHaveBeenCalledWith("Error: bad code", "error"));
  });

  it("falls back to credentials when loading the status fails", async () => {
    fetchMock.mockImplementation(async (url: string) => {
      if (url === "/api/telegram-monitor/status") throw new Error("down");
      if (url === "/api/telegram-monitor/sources") throw new Error("down");
      return ok({});
    });

    render(<TelegramMonitorCard />);
    expect(await screen.findByPlaceholderText("telegramMonitor.apiHashPlaceholder")).toBeInTheDocument();
    expect(screen.getByText("telegramMonitor.notAuthorized")).toBeInTheDocument();
  });

  it("falls back to credentials when the status payload is invalid", async () => {
    routes({
      "GET /api/telegram-monitor/status": () =>
        ok({
          ...statusBase,
          sync_interval_minutes: {
            toString() {
              throw new Error("bad interval");
            },
          },
        }),
    });

    render(<TelegramMonitorCard />);
    expect(await screen.findByPlaceholderText("telegramMonitor.apiHashPlaceholder")).toBeInTheDocument();
  });
});

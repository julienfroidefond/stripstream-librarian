import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// The shared setup mock returns a new router object on every call, which makes
// the page's `[router]` effect dependency unstable and loops forever under
// `act()`. Real `useRouter()` is stable across renders, so mirror that here.
const routerMock = vi.hoisted(() => ({
  push: vi.fn(),
  replace: vi.fn(),
  refresh: vi.fn(),
  back: vi.fn(),
  prefetch: vi.fn(),
}));
vi.mock("next/navigation", () => ({ useRouter: () => routerMock }));

import AnilistCallbackPage from "@/app/(app)/anilist/callback/page";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  routerMock.push.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  window.history.replaceState({}, "", "/anilist/callback");
});

function withToken(token = "tok123") {
  window.history.replaceState({}, "", `/anilist/callback#access_token=${token}`);
}

function isPost(init?: RequestInit) {
  return init?.method === "POST";
}

describe("AnilistCallbackPage", () => {
  it("reports a missing token without calling the API", async () => {
    render(<AnilistCallbackPage />);

    await waitFor(() =>
      expect(screen.getByText("settings.anilistCallbackNoToken")).toBeInTheDocument()
    );
    expect(screen.getByText("settings.anilistCallbackBackToSettings")).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("saves the token, stores the user id and shows the connected user", async () => {
    withToken();
    fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
      if (url === "/api/settings/anilist" && !isPost(init)) {
        return { ok: true, json: async () => ({ client_id: "existing" }) };
      }
      if (url === "/api/settings/anilist") return { ok: true };
      if (url === "/api/anilist/status") {
        return { ok: true, json: async () => ({ user_id: 42, username: "bob" }) };
      }
      throw new Error(`unexpected request: ${url}`);
    });

    render(<AnilistCallbackPage />);

    await waitFor(() =>
      expect(
        screen.getByText('settings.anilistCallbackConnected:{"username":"bob"}')
      ).toBeInTheDocument()
    );
    expect(screen.getByText("settings.anilistCallbackRedirecting")).toBeInTheDocument();

    // Token first, then the resolved user_id.
    const posts = fetchMock.mock.calls.filter(([, init]) => isPost(init as RequestInit | undefined));
    expect(posts).toHaveLength(2);
    expect(JSON.parse((posts[1][1] as RequestInit).body as string).value.user_id).toBe(42);
  });

  it("keeps the token when the user lookup fails", async () => {
    withToken();
    fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
      if (url === "/api/settings/anilist" && !isPost(init)) return { ok: true, json: async () => ({}) };
      if (url === "/api/settings/anilist") return { ok: true };
      return { ok: false };
    });

    render(<AnilistCallbackPage />);

    await waitFor(() =>
      expect(screen.getByText("settings.anilistCallbackTokenSaved")).toBeInTheDocument()
    );
  });

  it("surfaces a save failure", async () => {
    withToken();
    fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
      if (url === "/api/settings/anilist" && !isPost(init)) return { ok: true, json: async () => ({}) };
      return { ok: false };
    });

    render(<AnilistCallbackPage />);

    await waitFor(() =>
      expect(screen.getByText("settings.anilistCallbackSaveError")).toBeInTheDocument()
    );
    expect(screen.queryByText("settings.anilistCallbackRedirecting")).not.toBeInTheDocument();
  });
});

import { render, screen } from "@testing-library/react";
import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  listTokens: vi.fn(),
  createToken: vi.fn(),
  revokeToken: vi.fn(),
  deleteToken: vi.fn(),
  updateToken: vi.fn(),
  fetchUsers: vi.fn(),
  createUser: vi.fn(),
  deleteUser: vi.fn(),
  updateUser: vi.fn(),
  fetchAllGenres: vi.fn(),
  fetchUserGenreRestrictions: vi.fn(),
  setUserGenreRestrictions: vi.fn(),
}));

vi.mock("@/lib/api", () => api);
vi.mock("next/cache", () => ({ revalidatePath: vi.fn() }));
vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import { TokensTab } from "@/app/(app)/settings/components/TokensTab";

const tokens = [
  { id: "t1", name: "Admin", scope: "admin", user_id: null, prefix: "sl_admin", revoked_at: null },
  { id: "t2", name: "Orphan", scope: "read", user_id: null, prefix: "sl_orph", revoked_at: null },
  { id: "t3", name: "Reader", scope: "read", user_id: "u1", prefix: "sl_read", revoked_at: null },
  { id: "t4", name: "Old", scope: "read", user_id: "u1", prefix: "sl_old", revoked_at: "2024-01-01" },
];

const users = [
  { id: "u1", username: "alice", token_count: 2, books_read: 3, books_reading: 1, created_at: "2024-01-01T00:00:00Z" },
  { id: "u2", username: "bob", token_count: 0, books_read: 0, books_reading: 0, created_at: "2024-01-01T00:00:00Z" },
];

function setup() {
  api.listTokens.mockResolvedValue(tokens);
  api.fetchUsers.mockResolvedValue(users);
  api.fetchAllGenres.mockResolvedValue(["Action", "Drama"]);
  api.createToken.mockResolvedValue({ token: "sl_new" });
  api.fetchUserGenreRestrictions.mockImplementation((id: string) =>
    Promise.resolve({ blocked_genres: id === "u1" ? ["Drama"] : [] }),
  );
}

function collectActions(node: unknown, out: ((fd: FormData) => Promise<void>)[] = []) {
  if (Array.isArray(node)) {
    node.forEach((n) => collectActions(n, out));
    return out;
  }
  if (node && typeof node === "object" && "props" in node) {
    const props = (node as { props: Record<string, unknown> }).props;
    if (typeof props.action === "function") out.push(props.action as (fd: FormData) => Promise<void>);
    collectActions(props.children, out);
  }
  return out;
}

function fullFormData() {
  const fd = new FormData();
  fd.append("name", "new-token");
  fd.append("scope", "admin");
  fd.append("user_id", "u1");
  fd.append("id", "t1");
  fd.append("username", "carol");
  fd.append("blocked_genres", JSON.stringify(["Action"]));
  return fd;
}

beforeEach(() => {
  vi.clearAllMocks();
  setup();
});

describe("TokensTab", () => {
  it("renders users, tokens and the created token banner", async () => {
    render(await TokensTab({ createdToken: "sl_secret" }));
    expect(screen.getAllByText("alice").length).toBeGreaterThan(0);
    expect(screen.getAllByText("bob").length).toBeGreaterThan(0);
    expect(screen.getByText("Drama")).toBeInTheDocument();
    expect(screen.getByText("sl_secret")).toBeInTheDocument();
    expect(screen.getAllByText("tokens.noUser").length).toBeGreaterThan(0);
    expect(screen.getByText("tokens.revoked")).toBeInTheDocument();
    expect(screen.getAllByText("tokens.active").length).toBeGreaterThan(0);
  });

  it("handles empty data and unknown genres", async () => {
    api.listTokens.mockResolvedValue([]);
    api.fetchUsers.mockResolvedValue([]);
    api.fetchAllGenres.mockRejectedValue(new Error("nope"));
    render(await TokensTab({}));
    expect(screen.getByText("tokens.apiTokens")).toBeInTheDocument();
  });

  it("falls back to no restrictions when the lookup fails", async () => {
    api.fetchUserGenreRestrictions.mockRejectedValue(new Error("nope"));
    render(await TokensTab({}));
    expect(screen.queryByText("Drama")).not.toBeInTheDocument();
  });

  it("runs all server actions", async () => {
    const tree = await TokensTab({});
    const actions = [...new Set(collectActions(tree))];
    expect(actions.length).toBeGreaterThanOrEqual(6);

    for (const action of actions) {
      await action(fullFormData());
    }

    expect(api.createUser).toHaveBeenCalledWith("carol");
    expect(api.createToken).toHaveBeenCalledWith("new-token", "admin", "u1");
    expect(api.revokeToken).toHaveBeenCalledWith("t1");
    expect(api.deleteToken).toHaveBeenCalledWith("t1");
    expect(api.deleteUser).toHaveBeenCalledWith("t1");
    expect(api.updateUser).toHaveBeenCalledWith("t1", "carol");
    expect(api.setUserGenreRestrictions).toHaveBeenCalledWith("t1", ["Action"]);
    expect(api.updateToken).toHaveBeenCalledWith("t1", "u1");
    expect(redirect).toHaveBeenCalledWith(expect.stringContaining("/settings?tab=tokens&created="));
    expect(revalidatePath).toHaveBeenCalledWith("/settings");
  });

  it("skips actions with missing required fields", async () => {
    const tree = await TokensTab({});
    const actions = [...new Set(collectActions(tree))];
    const fd = new FormData();
    fd.append("id", "t1");
    for (const action of actions) {
      await action(fd);
    }
    expect(api.createUser).not.toHaveBeenCalled();
    expect(api.createToken).not.toHaveBeenCalled();
    expect(api.updateUser).not.toHaveBeenCalled();
    expect(api.setUserGenreRestrictions).toHaveBeenCalledWith("t1", []);
  });
});

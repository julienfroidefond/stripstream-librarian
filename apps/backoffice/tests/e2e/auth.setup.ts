import fs from "node:fs";
import path from "node:path";
import { test as setup, expect, type APIRequestContext } from "@playwright/test";

import { getAdminCredentials, loadEnvLocal } from "./env";
import { isLibrarySeedingEnabled, seedLibrary } from "./support/library";

const authFile = path.resolve(__dirname, ".auth/state.json");

/** Deterministic reader used by every spec so active-user-only UI renders. */
const READER_USERNAME = "e2e-reader";

/** Resources created by the suite all share this prefix, so stale runs can be cleaned. */
const CLEANUP_PREFIX = "e2e-";

type AdminUser = { id: string; username: string };
type ReadingList = { id: string; name: string };
type AdminToken = { id: string; name: string };

function resolveApi(): { baseUrl: string; token: string } {
  const env = loadEnvLocal();
  const baseUrl = process.env.API_BASE_URL || env.API_BASE_URL || "http://localhost:7080";
  const token = process.env.API_BOOTSTRAP_TOKEN || env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    throw new Error(
      "API_BOOTSTRAP_TOKEN introuvable. Ajoute-le à apps/backoffice/.env.local ou passe-le en variable d'environnement."
    );
  }
  return { baseUrl, token };
}

async function ensureReaderUser(
  request: APIRequestContext,
  baseUrl: string,
  token: string
): Promise<AdminUser> {
  const headers = { Authorization: `Bearer ${token}` };

  const listRes = await request.get(`${baseUrl}/admin/users`, { headers });
  expect(listRes.ok(), `liste des utilisateurs échouée (${listRes.status()})`).toBeTruthy();
  const users = (await listRes.json()) as AdminUser[];

  const existing = users.find((user) => user.username === READER_USERNAME);
  if (existing) return existing;

  // Fresh database: create a reader so the active-user UI (mark read, ratings,
  // reading progress) is rendered instead of the admin empty states.
  const createRes = await request.post(`${baseUrl}/admin/users`, {
    headers,
    data: { username: READER_USERNAME },
  });
  expect(createRes.ok(), `création de ${READER_USERNAME} échouée (${createRes.status()})`).toBeTruthy();
  return (await createRes.json()) as AdminUser;
}

/**
 * Best-effort cleanup of resources left behind by an interrupted run. Only
 * names carrying the suite prefix are touched, so real data is never removed.
 */
async function cleanupPreviousRuns(
  request: APIRequestContext,
  baseUrl: string,
  token: string
): Promise<void> {
  const headers = { Authorization: `Bearer ${token}` };

  const [listsRes, tokensRes, usersRes] = await Promise.all([
    request.get(`${baseUrl}/reading-lists`, { headers }),
    request.get(`${baseUrl}/admin/tokens`, { headers }),
    request.get(`${baseUrl}/admin/users`, { headers }),
  ]);

  if (listsRes.ok()) {
    const lists = (await listsRes.json()) as ReadingList[];
    for (const list of lists.filter((item) => item.name.startsWith(CLEANUP_PREFIX))) {
      await request.delete(`${baseUrl}/reading-lists/${list.id}`, { headers }).catch(() => undefined);
    }
  }

  if (tokensRes.ok()) {
    const tokens = (await tokensRes.json()) as AdminToken[];
    for (const token of tokens.filter((item) => item.name.startsWith(CLEANUP_PREFIX))) {
      await request.delete(`${baseUrl}/admin/tokens/${token.id}`, { headers }).catch(() => undefined);
      await request
        .post(`${baseUrl}/admin/tokens/${token.id}/delete`, { headers })
        .catch(() => undefined);
    }
  }

  if (usersRes.ok()) {
    const users = (await usersRes.json()) as AdminUser[];
    for (const user of users.filter((item) =>
      item.username.startsWith(`${CLEANUP_PREFIX}user-`)
    )) {
      await request.delete(`${baseUrl}/admin/users/${user.id}`, { headers }).catch(() => undefined);
    }
  }
}

setup("authenticate", async ({ request }) => {
  const { username, password } = getAdminCredentials();

  const res = await request.post("/api/auth/login", {
    data: { username, password },
  });
  expect(res.ok(), `login échoué (${res.status()})`).toBeTruthy();

  const { baseUrl, token } = resolveApi();
  const reader = await ensureReaderUser(request, baseUrl, token);
  await cleanupPreviousRuns(request, baseUrl, token);

  if (isLibrarySeedingEnabled()) {
    await seedLibrary(request, baseUrl, token);
  }

  await request.storageState({ path: authFile });

  // Simulate an active reading user so active-user-only UI (read buttons,
  // ratings, etc.) is rendered in the specs. The API validates X-As-User as a
  // UUID, so a placeholder string would 400 every request.
  const state = JSON.parse(fs.readFileSync(authFile, "utf8")) as {
    cookies: Array<Record<string, unknown>>;
    origins: unknown[];
  };
  const base = new URL(process.env.E2E_BASE_URL || "http://localhost:7082");
  state.cookies = state.cookies.filter((cookie) => cookie.name !== "as_user_id");
  state.cookies.push({
    name: "as_user_id",
    value: reader.id,
    domain: base.hostname,
    path: "/",
    expires: -1,
    httpOnly: false,
    secure: base.protocol === "https:",
    sameSite: "Lax",
  });
  fs.writeFileSync(authFile, JSON.stringify(state, null, 2));
});

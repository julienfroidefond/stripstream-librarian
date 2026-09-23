import fs from "node:fs";
import path from "node:path";
import { test as setup, expect } from "@playwright/test";
import { getAdminCredentials, loadEnvLocal } from "./env";

const authFile = path.resolve(__dirname, ".auth/state.json");

setup("authenticate", async ({ request }) => {
  const { username, password } = getAdminCredentials();

  const res = await request.post("/api/auth/login", {
    data: { username, password },
  });
  expect(res.ok(), `login échoué (${res.status()})`).toBeTruthy();

  await request.storageState({ path: authFile });

  // Simulate an active reading user so active-user-only UI (read buttons,
  // ratings, etc.) is rendered in the smoke tests. The API validates
  // X-As-User as a UUID, so a placeholder string would 400 every request.
  // The backoffice has no /api/admin/users proxy route, so hit the API directly.
  const env = loadEnvLocal();
  const apiBaseUrl = process.env.API_BASE_URL || env.API_BASE_URL || "http://localhost:7080";
  const bootstrapToken = process.env.API_BOOTSTRAP_TOKEN || env.API_BOOTSTRAP_TOKEN;
  if (!bootstrapToken) {
    throw new Error(
      "API_BOOTSTRAP_TOKEN introuvable. Ajoute-le à apps/backoffice/.env.local ou passe-le en variable d'environnement."
    );
  }

  const usersRes = await request.get(`${apiBaseUrl}/admin/users`, {
    headers: { Authorization: `Bearer ${bootstrapToken}` },
  });
  expect(usersRes.ok(), `liste des utilisateurs échouée (${usersRes.status()})`).toBeTruthy();
  const users: { id: string }[] = await usersRes.json();
  const activeUserId = users[0]?.id;
  if (!activeUserId) {
    throw new Error("Aucun utilisateur en base : impossible de définir as_user_id pour les tests e2e.");
  }

  const state = JSON.parse(fs.readFileSync(authFile, "utf8"));
  state.cookies.push({
    name: "as_user_id",
    value: activeUserId,
    domain: "localhost",
    path: "/",
    expires: -1,
    httpOnly: false,
    secure: false,
    sameSite: "Lax",
  });
  fs.writeFileSync(authFile, JSON.stringify(state, null, 2));
});

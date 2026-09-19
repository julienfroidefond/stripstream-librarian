import fs from "node:fs";
import path from "node:path";
import { test as setup, expect } from "@playwright/test";
import { getAdminCredentials } from "./env";

const authFile = path.resolve(__dirname, ".auth/state.json");

setup("authenticate", async ({ request }) => {
  const { username, password } = getAdminCredentials();

  const res = await request.post("/api/auth/login", {
    data: { username, password },
  });
  expect(res.ok(), `login échoué (${res.status()})`).toBeTruthy();

  await request.storageState({ path: authFile });

  // Simulate an active reading user so active-user-only UI (read buttons,
  // ratings, etc.) is rendered in the smoke tests.
  const state = JSON.parse(fs.readFileSync(authFile, "utf8"));
  state.cookies.push({
    name: "as_user_id",
    value: "e2e-user",
    domain: "localhost",
    path: "/",
    expires: -1,
    httpOnly: false,
    secure: false,
    sameSite: "Lax",
  });
  fs.writeFileSync(authFile, JSON.stringify(state, null, 2));
});

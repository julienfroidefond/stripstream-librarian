import { test, expect } from "@playwright/test";

import {
  createToken,
  deleteTokensByName,
  deleteUsersByUsername,
  findTokenByName,
  findUserByUsername,
} from "../support/api";
import { uniqueName } from "../helpers";

test.describe("admin — tokens API", () => {
  test("crée, révoque puis supprime un token", async ({ page }) => {
    const name = uniqueName("e2e-token");

    try {
      await page.goto("/settings?tab=tokens");
      await expect(page.locator("main h1")).toContainText("Settings");
      await expect(page.getByText("API Tokens", { exact: true }).first()).toBeVisible();

      const createCard = page
        .getByText("Create a new token", { exact: true })
        .locator("xpath=ancestor::section[1]");
      await createCard.getByPlaceholder("Token name").fill(name);
      // A read-scoped token must be attached to a reader.
      await createCard.locator('select[name="user_id"]').selectOption({ label: "e2e-reader" });
      await createCard.getByRole("button", { name: "Create token" }).click();

      // The server action redirects with the plaintext token in the URL.
      await expect(page).toHaveURL(/created=stl_/);
      await expect(page.getByText("Token created")).toBeVisible();
      const tokenValue = (await page.locator("main pre").first().textContent())?.trim();
      expect(tokenValue).toMatch(/^stl_/);

      const row = page.getByRole("row").filter({ hasText: name });
      await expect(row).toBeVisible();
      await expect(row.getByText("Active", { exact: true })).toBeVisible();

      await row.getByRole("button", { name: "Revoke" }).click();
      await expect(row.getByText("Revoked", { exact: true })).toBeVisible();

      await row.getByRole("button", { name: "Delete", exact: true }).click();
      await expect(page.getByRole("row").filter({ hasText: name })).toHaveCount(0);
    } finally {
      await deleteTokensByName(name).catch(() => undefined);
    }
  });
});

test.describe("admin — tokens et utilisateurs", () => {
  test("réassigne un token et renomme un lecteur en ligne", async ({ page }) => {
    const tokenName = uniqueName("e2e-token-reassign");
    const username = uniqueName("e2e-rename-user");
    const renamed = uniqueName("e2e-renamed-user");
    const reader = await findUserByUsername("e2e-reader");
    test.skip(!reader, "Le lecteur e2e-reader est créé par le setup");

    try {
      await page.goto("/settings?tab=tokens");
      await expect(page.getByText("API Tokens", { exact: true }).first()).toBeVisible();

      // Creating the reader through the UI runs the server action and its
      // `revalidatePath("/settings")`; a direct API create would leave the
      // 60-second `fetchUsers()` cache without the new option.
      const usersTable = page.locator("main table").first();
      await page.getByPlaceholder("Username").fill(username);
      await page.getByRole("button", { name: "Create", exact: true }).click();
      await expect(usersTable.getByRole("row").filter({ hasText: username })).toBeVisible();

      const user = await findUserByUsername(username);
      if (!user) throw new Error(`utilisateur ${username} introuvable après création`);

      // Tokens are fetched without cache; only the page needs a reload to
      // render a fixture created after its first render.
      const token = await createToken(tokenName, reader!.id);
      await page.reload();
      await expect(page.getByText("API Tokens", { exact: true }).first()).toBeVisible();

      // Reassign the token to the disposable reader through the inline select.
      const tokenRow = page.getByRole("row").filter({ hasText: tokenName });
      await expect(tokenRow).toBeVisible();
      await tokenRow.locator("select").selectOption({ label: username });

      await expect
        .poll(async () => (await findTokenByName(tokenName))?.user_id)
        .toBe(user.id);

      // Rename that reader inline in the users table.
      const userRow = usersTable.getByRole("row").filter({ hasText: username });
      await expect(userRow).toBeVisible();
      await userRow.getByTitle("Edit").click();
      // Once editing starts the row no longer contains the username as text,
      // so target the only inline input in the users table.
      const nameInput = usersTable.getByRole("textbox");
      await expect(nameInput).toBeVisible();
      await nameInput.fill(renamed);
      await nameInput.press("Enter");

      await expect
        .poll(async () => (await findUserByUsername(renamed))?.id ?? null)
        .toBe(user.id);
      await expect(
        usersTable.getByRole("row").filter({ hasText: renamed })
      ).toBeVisible();
      expect(token.id).toBeTruthy();
    } finally {
      await deleteTokensByName(tokenName).catch(() => undefined);
      await deleteUsersByUsername(renamed).catch(() => undefined);
      await deleteUsersByUsername(username).catch(() => undefined);
    }
  });
});

test.describe("admin — utilisateurs", () => {
  test("crée puis supprime un utilisateur lecteur", async ({ page }) => {
    const username = uniqueName("e2e-user");

    try {
      await page.goto("/settings?tab=tokens");
      await expect(page.getByText("Users", { exact: true }).first()).toBeVisible();

      await page.getByPlaceholder("Username").fill(username);
      await page.getByRole("button", { name: "Create", exact: true }).click();

      // The token table also lists usernames in its assignment <select>, so
      // scope the row query to the first table (users).
      const usersTable = page.locator("main table").first();
      const row = usersTable.getByRole("row").filter({ hasText: username });
      await expect(row).toBeVisible();

      await row.getByRole("button", { name: "Delete", exact: true }).click();
      await expect(usersTable.getByRole("row").filter({ hasText: username })).toHaveCount(0);
    } finally {
      await deleteUsersByUsername(username).catch(() => undefined);
    }
  });
});

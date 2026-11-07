import { test, expect } from "@playwright/test";

import { deleteTokensByName, deleteUsersByUsername } from "../support/api";
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

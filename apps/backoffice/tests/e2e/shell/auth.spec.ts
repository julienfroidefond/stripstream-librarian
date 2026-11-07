import { test, expect } from "@playwright/test";

import { getAdminCredentials } from "../env";

test.describe("auth — protection des routes et formulaire de connexion", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("une page protégée redirige vers /login", async ({ page }) => {
    await page.goto("/series");

    await expect(page).toHaveURL(/\/login/);
    await expect(page.getByRole("heading", { level: 1 })).toContainText("StripStream");
    await expect(page.getByLabel("Identifiant")).toBeVisible();
    await expect(page.getByLabel("Mot de passe")).toBeVisible();
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
  });

  test("des identifiants invalides affichent une erreur", async ({ page }) => {
    await page.goto("/login");

    await page.getByLabel("Identifiant").fill("e2e-nobody");
    await page.getByLabel("Mot de passe").fill("not-the-password");
    await page.getByRole("button", { name: "Se connecter" }).click();

    await expect(page.getByText(/invalid credentials/i)).toBeVisible();
    await expect(page).toHaveURL(/\/login/);
  });

  test("des identifiants valides ouvrent le dashboard", async ({ page }) => {
    const { username, password } = getAdminCredentials();

    await page.goto("/login");
    await page.getByLabel("Identifiant").fill(username);
    await page.getByLabel("Mot de passe").fill(password);
    await page.getByRole("button", { name: "Se connecter" }).click();

    await expect(page).toHaveURL(/\/$/);
    await expect(page.locator("main h1")).toContainText("Dashboard");
  });
});

test.describe("auth — déconnexion", () => {
  test("le bouton Log out ramène sur /login", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("main h1")).toContainText("Dashboard");

    await page.locator('header button[title="Log out"]').click();

    await expect(page).toHaveURL(/\/login/);
    await expect(page.getByRole("button", { name: "Se connecter" })).toBeVisible();
  });
});

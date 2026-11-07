import { test, expect } from "@playwright/test";

const TABS: Array<{ id: string; label: string; marker?: string }> = [
  { id: "general", label: "General" },
  { id: "media", label: "Images & Cache" },
  { id: "downloadTools", label: "Download Tools" },
  { id: "metadata", label: "Metadata" },
  { id: "ai", label: "AI" },
  { id: "readingStatus", label: "Reading Status" },
  { id: "notifications", label: "Notifications" },
  { id: "tokens", label: "Tokens", marker: "API Tokens" },
  { id: "archives", label: "Archives", marker: "No archived series." },
  { id: "orphans", label: "Orphan series", marker: "Orphan series" },
];

test.describe("paramètres — navigation par onglets", () => {
  test("chaque onglet met à jour l'URL et son contenu", async ({ page }) => {
    await page.goto("/settings");

    await expect(page.locator("main h1")).toContainText("Settings");
    const nav = page.locator('nav[aria-label="Settings sections"]');
    await expect(nav).toBeVisible();

    for (const tab of TABS) {
      const button = nav.getByRole("button", { name: tab.label, exact: true });
      await button.click();

      await expect(page).toHaveURL(new RegExp(`tab=${tab.id}(&|$)`));
      await expect(button).toHaveAttribute("aria-current", "page");

      if (tab.marker) {
        await expect(page.locator("main").getByText(tab.marker, { exact: true }).first()).toBeVisible();
      }
    }
  });

  test("l'onglet Download Tools expose des tests de connexion", async ({ page }) => {
    await page.goto("/settings?tab=downloadTools");

    const buttons = page.getByRole("button", { name: "Test connection" });
    await expect(buttons.first()).toBeVisible();
    expect(await buttons.count()).toBeGreaterThan(0);
  });
});

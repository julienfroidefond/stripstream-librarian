import { test, expect } from "@playwright/test";

const TABS: Array<{ id: string; label: string; marker?: string | RegExp }> = [
  { id: "general", label: "General" },
  { id: "media", label: "Images & Cache" },
  { id: "downloadTools", label: "Download Tools" },
  { id: "metadata", label: "Metadata" },
  { id: "ai", label: "AI" },
  { id: "readingStatus", label: "Reading Status" },
  { id: "notifications", label: "Notifications" },
  { id: "tokens", label: "Tokens", marker: "API Tokens" },
  // Match both the empty state and the count description so transient archives
  // created by the metadata fixtures never make this tab flaky.
  { id: "archives", label: "Archives", marker: /archived series/i },
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
        const options = typeof tab.marker === "string" ? { exact: true } : undefined;
        await expect(page.locator("main").getByText(tab.marker, options).first()).toBeVisible();
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

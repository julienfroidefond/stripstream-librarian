import { test, expect } from "@playwright/test";

test.describe("dashboard — vue d'ensemble", () => {
  test("affiche les cartes de statistiques et les répartitions", async ({ page }) => {
    await page.goto("/");

    const main = page.locator("main");
    await expect(main.locator("h1")).toContainText("Dashboard");

    for (const label of ["Books", "Series", "Libraries", "Pages", "Authors", "Total size"]) {
      await expect(main.getByText(label, { exact: true }).first()).toBeVisible();
    }

    for (const section of [
      "Reading status",
      "By format",
      "Metadata coverage",
      "Reading activity",
      "Job runs",
      "Books added",
    ]) {
      await expect(main.getByText(section, { exact: true }).first()).toBeVisible();
    }
  });

  test("le sélecteur de période des graphiques est utilisable", async ({ page }) => {
    await page.goto("/");

    const main = page.locator("main");
    await expect(main.getByRole("button", { name: "Day" }).first()).toBeVisible();
    await main.getByRole("button", { name: "Week" }).first().click();
    await expect(main.getByRole("button", { name: "Week" }).first()).toBeVisible();
    await main.getByRole("button", { name: "Month" }).first().click();
    await expect(main.getByRole("button", { name: "Month" }).first()).toBeVisible();
  });
});

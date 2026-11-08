import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import {
  deleteMetadataLink,
  getSeriesMetadata,
  listMetadataLinksForSeries,
} from "../support/api";
import {
  createMetadataSeries,
  openMetadataSeries,
  removeMetadataSeries,
} from "../support/metadata";

test.describe("métadonnées — recherche externe", () => {
  test("approuve puis délie un résultat de recherche simulé", async ({ page }) => {
    test.setTimeout(90_000);

    const series = await createMetadataSeries();
    const provider = "google_books";
    const externalId = uniqueName("e2e-ext");
    const candidateTitle = uniqueName("Provider series");
    const author = uniqueName("Provider author");
    const description = uniqueName("Description importée par le test e2e");
    const candidate = {
      provider,
      external_id: externalId,
      title: candidateTitle,
      authors: [author],
      description,
      publishers: ["E2E Publisher"],
      start_year: 2024,
      total_volumes: 1,
      cover_url: null,
      external_url: null,
      confidence: 0.92,
      metadata_json: {
        description,
        authors: [author],
        publishers: ["E2E Publisher"],
        start_year: 2024,
      },
    };

    await page.route("**/api/metadata/search", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([candidate]),
      })
    );

    try {
      await openMetadataSeries(page, series);

      await page.getByRole("button", { name: "More", exact: true }).click();
      await page.getByRole("menuitem", { name: /Search metadata/ }).click();

      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await expect(panel.getByText(candidateTitle, { exact: true })).toBeVisible();
      await panel.getByText(candidateTitle, { exact: true }).click();
      await expect(panel.getByText("How would you like to sync?")).toBeVisible();

      await panel
        .getByRole("button")
        .filter({ hasText: "Sync series only" })
        .click();
      await expect(panel.getByText("Metadata synced successfully!")).toBeVisible({
        timeout: 20_000,
      });

      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).authors)
        .toContain(author);
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).description)
        .toBe(description);

      await panel.getByRole("button", { name: "Close", exact: true }).click();
      await expect(panel).toBeHidden();

      // Reload so the server component passes the approved link to the modal.
      await page.reload();
      await expect(page.locator("main h1")).toHaveText(series.name);
      await page.getByRole("button", { name: "More", exact: true }).click();
      await page.getByRole("menuitem", { name: /Metadata/ }).click();
      await expect(panel.getByText("Linked providers")).toBeVisible();

      const unlinkResponse = page.waitForResponse(
        (response) =>
          response.request().method() === "DELETE" &&
          new URL(response.url()).pathname === "/api/metadata/links"
      );
      await panel.getByRole("button", { name: "Unlink", exact: true }).first().click();
      expect((await unlinkResponse).status()).toBe(200);
      await expect
        .poll(async () => listMetadataLinksForSeries(series.seriesId))
        .toEqual([]);
      await expect(panel.getByRole("button", { name: "Unlink" })).toHaveCount(0);
    } finally {
      const links = await listMetadataLinksForSeries(series.seriesId).catch(() => []);
      await Promise.all(links.map((link) => deleteMetadataLink(link.id).catch(() => undefined)));
      await removeMetadataSeries(series);
    }
  });
});

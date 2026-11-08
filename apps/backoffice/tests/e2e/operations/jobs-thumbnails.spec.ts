import { test, expect } from "@playwright/test";

import { launchFromPalette, selectJobsLibrary, waitForTerminalJob } from "../support/jobs";

/**
 * Génération et régénération des miniatures sur la bibliothèque vide `e2e-jobs`.
 *
 * La génération des manquantes est une action rapide ; la régénération complète
 * demande une confirmation explicite. Les deux jobs ne touchent aucun provider
 * externe et restent isolés des autres workers.
 */

test.describe("jobs d'indexation — miniatures", () => {
  test("génère les miniatures manquantes puis régénère toutes les miniatures", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    const library = await selectJobsLibrary(page);

    // Action rapide : aucun dialogue de confirmation.
    let jobId = await launchFromPalette(page, "Generate thumbnails");
    let job = await waitForTerminalJob(jobId);
    expect(job.type).toBe("thumbnail_rebuild");
    expect(job.library_id).toBe(library.id);

    // Régénération complète : confirmation puis lancement.
    await page.goto(`/jobs?library=${library.id}`);
    await page.getByRole("button", { name: /All actions/ }).click();
    const dialog = page.getByRole("dialog", { name: "All actions" });
    await dialog.getByPlaceholder("Search for an action…").fill("Regenerate thumbnails");
    await dialog.getByRole("button", { name: /Regenerate thumbnails/ }).click();
    await expect(dialog.getByRole("heading", { name: "Confirm launch" })).toBeVisible();
    await expect(dialog.getByText("Recreate all thumbnails")).toBeVisible();
    await dialog.getByRole("button", { name: "Launch action", exact: true }).click();
    await page.waitForURL(
      (url) => url.pathname === "/jobs" && url.searchParams.has("highlight"),
      { timeout: 30_000 }
    );
    jobId = new URL(page.url()).searchParams.get("highlight")!;
    job = await waitForTerminalJob(jobId);
    expect(job.type).toBe("thumbnail_regenerate");
    expect(job.library_id).toBe(library.id);
  });
});

import { test, expect } from "@playwright/test";

import { launchFromPalette, selectJobsLibrary, waitForTerminalJob } from "../support/jobs";

/**
 * Palette d'actions complète : recherche, confirmation explicite, annulation et
 * lancement de jobs non destructifs sur la bibliothèque isolée `e2e-jobs`.
 *
 * Les actions liées aux intégrations externes (Prowlarr, DDL, Telegram,
 * AniList) ne sont pas lancées : elles ne sont visibles que si l'intégration
 * est configurée et contacteraient un service réel. Les actions globales
 * (import des notes, synchronisation Telegram) sont également exclues.
 */

test.describe("jobs d'indexation — palette d'actions", () => {
  test("confirme ou annule puis lance rescan, rebuild complet et batch metadata", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    const library = await selectJobsLibrary(page);

    // Confirmation annulée : aucun job ne doit être créé.
    await page.getByRole("button", { name: /All actions/ }).click();
    let dialog = page.getByRole("dialog", { name: "All actions" });
    await dialog.getByPlaceholder("Search for an action…").fill("Regenerate thumbnails");
    await dialog.getByRole("button", { name: /Regenerate thumbnails/ }).click();
    await expect(dialog.getByRole("heading", { name: "Confirm launch" })).toBeVisible();
    await expect(dialog.getByText("Recreate all thumbnails")).toBeVisible();
    await expect(dialog.getByText(/irreversible or heavy/)).toBeVisible();
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog.getByPlaceholder("Search for an action…")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();

    // Rescan via la palette.
    let jobId = await launchFromPalette(page, "Deep rescan");
    let job = await waitForTerminalJob(jobId);
    expect(job.type).toBe("rescan");
    expect(job.library_id).toBe(library.id);

    // Rebuild complet confirmé explicitement.
    await page.goto(`/jobs?library=${library.id}`);
    await page.getByRole("button", { name: /All actions/ }).click();
    dialog = page.getByRole("dialog", { name: "All actions" });
    await dialog.getByPlaceholder("Search for an action…").fill("Full rebuild");
    await dialog.getByRole("button", { name: /Full rebuild/ }).click();
    await expect(dialog.getByRole("heading", { name: "Confirm launch" })).toBeVisible();
    await expect(dialog.getByText("Delete all & re-scan from scratch")).toBeVisible();
    await dialog.getByRole("button", { name: "Launch action", exact: true }).click();
    await page.waitForURL(
      (url) => url.pathname === "/jobs" && url.searchParams.has("highlight"),
      { timeout: 30_000 }
    );
    jobId = new URL(page.url()).searchParams.get("highlight")!;
    job = await waitForTerminalJob(jobId);
    expect(job.type).toBe("full_rebuild");
    expect(job.library_id).toBe(library.id);

    // Batch metadata sur une bibliothèque vide : job immédiat et sans réseau.
    await page.goto(`/jobs?library=${library.id}`);
    jobId = await launchFromPalette(page, "Auto-match unlinked series");
    job = await waitForTerminalJob(jobId);
    expect(job.type).toBe("metadata_batch");
    expect(job.library_id).toBe(library.id);
  });
});

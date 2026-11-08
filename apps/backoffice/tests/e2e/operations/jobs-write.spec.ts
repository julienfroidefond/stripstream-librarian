import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import { getIndexJob, type IndexJobSummary } from "../support/api";
import { ensureMetadataLibrary, METADATA_LIBRARY_NAME } from "../support/metadata";

const TERMINAL_STATUSES = new Set(["success", "partial", "failed", "cancelled"]);

async function waitForTerminalJob(id: string): Promise<IndexJobSummary> {
  let job: IndexJobSummary | null = null;
  await expect
    .poll(
      async () => {
        job = await getIndexJob(id);
        return TERMINAL_STATUSES.has(job.status);
      },
      { timeout: 45_000, intervals: [500, 1000, 2000] }
    )
    .toBe(true);
  return job!;
}

/**
 * Lancement d'un job depuis la barre d'actions rapides puis replay.
 *
 * Le rebuild ciblé de la bibliothèque temporaire est vide : il se termine
 * rapidement, ce qui rend le bouton Replay disponible sans attendre une vraie
 * indexation. Le replay crée un second job, vérifié à la fois par l'API et
 * dans la liste rafraîchie.
 */

test.describe("jobs d'indexation — écritures", () => {
  test("lance un rebuild ciblé puis rejoue le job", async ({ page }) => {
    test.setTimeout(90_000);
    const library = await ensureMetadataLibrary();

    // La bibliothèque `e2e-metadata` change d'identifiant à chaque run et
    // `fetchLibraries` est mis en cache : on réchauffe ce cache via la liste
    // des séries jusqu'à ce que la nouvelle bibliothèque y apparaisse.
    await expect(async () => {
      await page.goto(`/series?e2e=${encodeURIComponent(uniqueName("jobs"))}`);
      await expect(
        page.locator(`select[name="library"] option[value="${library.id}"]`)
      ).toHaveCount(1, { timeout: 1500 });
    }).toPass({ timeout: 45_000 });

    await page.goto("/jobs");
    const emptyTrigger = page.getByRole("button", { name: "Library: Choose a library" });
    await emptyTrigger.click();
    const picker = page.getByRole("dialog", { name: "Libraries" });
    await picker.getByRole("checkbox", { name: METADATA_LIBRARY_NAME }).check();
    await picker.getByRole("button", { name: "Done", exact: true }).click();
    await expect(
      page.getByRole("button", { name: `Library: ${METADATA_LIBRARY_NAME}`, exact: true })
    ).toBeVisible();

    await page.getByRole("button", { name: "Rebuild", exact: true }).click();
    await page.waitForURL(
      (url) => url.pathname === "/jobs" && url.searchParams.has("highlight"),
      { timeout: 30_000 }
    );
    const jobId = new URL(page.url()).searchParams.get("highlight");
    expect(jobId).toBeTruthy();

    const launched = await waitForTerminalJob(jobId!);
    expect(launched.library_id).toBe(library.id);
    expect(launched.type).toBe("rebuild");

    // Recharge la liste avec un job terminé : le bouton Replay n'apparaît que
    // hors statut actif.
    await page.goto(`/jobs?highlight=${jobId}&library=${library.id}`);
    const jobRow = page
      .locator("tbody tr")
      .filter({ has: page.locator(`a[href="/jobs/${jobId}"]`) });
    await expect(jobRow).toBeVisible();
    const replayButton = jobRow.getByRole("button", { name: "Replay", exact: true });
    await expect(replayButton).toBeVisible();

    const [replayResponse] = await Promise.all([
      page.waitForResponse(
        (response) =>
          response.request().method() === "POST" &&
          new URL(response.url()).pathname === `/api/jobs/${jobId}/replay`
      ),
      replayButton.click(),
    ]);
    expect(replayResponse.ok()).toBe(true);
    const replayed = (await replayResponse.json()) as IndexJobSummary;
    expect(replayed.id).not.toBe(jobId);

    const replayedJob = await waitForTerminalJob(replayed.id);
    expect(replayedJob.library_id).toBe(library.id);
    expect(replayedJob.type).toBe("rebuild");

    await page.goto(`/jobs?highlight=${replayed.id}&library=${library.id}`);
    const replayedRow = page
      .locator("tbody tr")
      .filter({ has: page.locator(`a[href="/jobs/${replayed.id}"]`) });
    await expect(replayedRow).toBeVisible();
    await expect(replayedRow.getByRole("button", { name: "Replay", exact: true })).toBeVisible();
  });
});

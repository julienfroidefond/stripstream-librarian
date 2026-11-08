import { expect, type Page } from "@playwright/test";

import { uniqueName } from "../helpers";
import { getIndexJob, type IndexJobSummary } from "./api";
import { ensureJobsLibrary, JOBS_LIBRARY_NAME } from "./metadata";

const TERMINAL_STATUSES = new Set(["success", "partial", "failed", "cancelled"]);

/** Poll the API until an indexer job reaches a terminal state. */
export async function waitForTerminalJob(id: string): Promise<IndexJobSummary> {
  let job: IndexJobSummary | null = null;
  await expect
    .poll(
      async () => {
        job = await getIndexJob(id);
        return TERMINAL_STATUSES.has(job.status);
      },
      { timeout: 60_000, intervals: [500, 1000, 2000] }
    )
    .toBe(true);
  return job!;
}

/**
 * Open `/jobs` scoped to the disposable `e2e-jobs` library.
 *
 * `fetchLibraries` is cached, so the spec first waits for the library recreated
 * by the setup to appear in the series filters before opening the jobs page.
 */
export async function selectJobsLibrary(page: Page) {
  const library = await ensureJobsLibrary();

  await expect(async () => {
    await page.goto(`/series?e2e=${encodeURIComponent(uniqueName("jobs-actions"))}`);
    await expect(
      page.locator(`select[name="library"] option[value="${library.id}"]`)
    ).toHaveCount(1, { timeout: 1500 });
  }).toPass({ timeout: 45_000 });

  await page.goto("/jobs");
  await page.getByRole("button", { name: "Library: Choose a library" }).click();
  const picker = page.getByRole("dialog", { name: "Libraries" });
  await picker.getByRole("checkbox", { name: JOBS_LIBRARY_NAME }).check();
  await picker.getByRole("button", { name: "Done", exact: true }).click();
  await expect(
    page.getByRole("button", { name: `Library: ${JOBS_LIBRARY_NAME}`, exact: true })
  ).toBeVisible();

  return library;
}

/** Search the actions palette, launch the matching action and return its job id. */
export async function launchFromPalette(page: Page, query: string): Promise<string> {
  await page.getByRole("button", { name: /All actions/ }).click();
  const dialog = page.getByRole("dialog", { name: "All actions" });
  await dialog.getByPlaceholder("Search for an action…").fill(query);
  await dialog.getByRole("button", { name: new RegExp(query) }).click();
  await page.waitForURL(
    (url) => url.pathname === "/jobs" && url.searchParams.has("highlight"),
    { timeout: 30_000 }
  );
  const jobId = new URL(page.url()).searchParams.get("highlight");
  expect(jobId).toBeTruthy();
  return jobId!;
}

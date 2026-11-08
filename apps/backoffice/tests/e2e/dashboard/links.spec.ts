import { test, expect, type Page } from "@playwright/test";

import { findUserByUsername, isDownloadsEnabled } from "../support/api";
import { createDisposableSeries, removeDisposableSeries } from "../support/books";

async function setReadingProgress(
  page: Page,
  bookId: string,
  status: "reading" | "read",
  currentPage?: number
) {
  const body: { status: string; current_page?: number } = { status };
  if (currentPage !== undefined) body.current_page = currentPage;
  const response = await page.request.patch(`/api/books/${bookId}/progress`, {
    data: body,
  });
  expect(response.ok()).toBeTruthy();
}

test.describe("dashboard — liens", () => {
  test("les cartes d'accès rapide mènent aux domaines", async ({ page }) => {
    const links = [
      { label: "Libraries", path: "/libraries", heading: "Libraries" },
      { label: "Books", path: "/books", heading: "Books" },
      { label: "Series", path: "/series", heading: "Series" },
      { label: "Jobs", path: "/jobs", heading: "Indexing jobs" },
    ] as const;

    for (const link of links) {
      await page.goto("/");
      const main = page.locator("main");
      await expect(main.locator("h1")).toContainText("Dashboard");

      const card = main.locator(`a[href="${link.path}"]`).filter({
        hasText: link.label,
      }).first();
      await expect(card).toBeVisible();
      await card.click();

      await expect(page).toHaveURL(new RegExp(`${link.path}$`));
      await expect(page.locator("main h1")).toContainText(link.heading);
    }
  });

  test("les lectures en cours/récentes et le filtre lecteur sont interactifs", async ({
    page,
  }) => {
    test.setTimeout(90_000);

    const reader = await findUserByUsername("e2e-reader");
    if (!reader) {
      test.skip(true, "lecteur e2e-reader introuvable");
      return;
    }

    const series = await createDisposableSeries(2);
    const readingBook = series.books[0];
    const readBook = series.books[1];

    try {
      await setReadingProgress(page, readingBook.id, "reading", 1);
      await setReadingProgress(page, readBook.id, "read");
      await page.goto("/");

      const main = page.locator("main");
      const currentlyReading = main.getByText("Currently reading", { exact: true });
      await expect(currentlyReading).toBeVisible({ timeout: 15_000 });

      const readingLink = main.locator(`a[href="/books/${readingBook.id}"]`).first();
      await expect(readingLink).toBeVisible();
      await readingLink.click();
      await expect(page).toHaveURL((url) => url.pathname === `/books/${readingBook.id}`);

      await page.goto("/");

      const recentlyRead = main.getByText("Recently read", { exact: true });
      await expect(recentlyRead).toBeVisible({ timeout: 15_000 });
      await expect(main.locator(`a[href="/books/${readBook.id}"]`).first()).toBeVisible();

      const card = recentlyRead.locator(
        "xpath=ancestor::div[contains(@class,'rounded')][1]"
      );
      const userPill = card.getByRole("button", { name: reader.username, exact: true });
      await expect(userPill).not.toHaveClass(/bg-primary/);
      await userPill.click();
      await expect(userPill).toHaveClass(/bg-primary/);

      const allPill = card.getByRole("button", { name: "All", exact: true });
      await allPill.click();
      await expect(allPill).toHaveClass(/bg-primary/);
      await expect(userPill).not.toHaveClass(/bg-primary/);
    } finally {
      await removeDisposableSeries(series);
    }
  });

  test("un téléchargement récent mène à la page des téléchargements", async ({ page }) => {
    if (!(await isDownloadsEnabled())) {
      test.skip(true, "téléchargements désactivés");
      return;
    }

    await page.goto("/");
    const heading = page.getByText("Recent downloads", { exact: true });
    await heading.waitFor({ state: "visible", timeout: 10_000 }).catch(() => undefined);
    const downloadLink = page.locator('main a[href="/downloads"]').first();
    if ((await downloadLink.count()) === 0) {
      test.skip(true, "aucun téléchargement récent");
      return;
    }

    await downloadLink.click();
    await expect(page).toHaveURL((url) => url.pathname === "/downloads");
  });
});

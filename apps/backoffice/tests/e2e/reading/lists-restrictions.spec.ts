import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import {
  addSeriesToReadingList,
  createReadingList,
  createUser,
  deleteReadingList,
  deleteUsersByUsername,
  getReadingListForUser,
  setGenreRestrictions,
} from "../support/api";
import {
  createMetadataSeries,
  removeMetadataSeries,
  type MetadataSeries,
} from "../support/metadata";

/**
 * Une liste de lecture doit appliquer les restrictions de genres du lecteur
 * actif, tout en restant complète pour Admin.
 *
 * Le lecteur et sa restriction sont des fixtures API ; la vérification passe
 * par le rendu de la page de liste sous deux identités.
 */

const BASE_URL = process.env.E2E_BASE_URL || "http://localhost:7082";

test.describe("listes de lecture — restrictions de genres", () => {
  test("masque une série bloquée pour un lecteur mais la garde pour Admin", async ({ page }) => {
    const username = uniqueName("e2e-list-reader");
    const blockedGenre = uniqueName("E2E List Blocked");
    const visible = await createMetadataSeries();
    const restricted = await createMetadataSeries({ genres: [blockedGenre] });
    const list = await createReadingList(uniqueName("e2e-restricted-list"));
    let user: Awaited<ReturnType<typeof createUser>> | null = null;

    try {
      await addSeriesToReadingList(list.id, visible.seriesId);
      await addSeriesToReadingList(list.id, restricted.seriesId);
      user = await createUser(username);
      await setGenreRestrictions(user.id, [blockedGenre]);

      // Vérifie d'abord le contrat API consommé par le Server Component.
      const readerList = await getReadingListForUser(list.id, user.id);
      expect(readerList.items.map((item) => item.name)).toEqual([visible.name]);

      await page.context().addCookies([
        { name: "as_user_id", value: user.id, url: BASE_URL },
      ]);
      await page.goto(`/reading-lists/${list.id}?e2e=${encodeURIComponent(uniqueName("reader"))}`);
      await expect(page.locator("main h1")).toContainText(list.name);
      await expect(page.getByText(visible.name, { exact: true })).toBeVisible();
      await expect(page.getByText(restricted.name, { exact: true })).toHaveCount(0);
      await expect(page.getByText("1 series", { exact: true })).toBeVisible();
      await expect(page.getByText("No books", { exact: true })).toBeVisible();

      // Admin (aucun utilisateur actif) voit les deux séries.
      await page.context().clearCookies({ name: "as_user_id" });
      await page.goto(`/reading-lists/${list.id}?e2e=${encodeURIComponent(uniqueName("admin"))}`);
      await expect(page.getByText(visible.name, { exact: true })).toBeVisible();
      await expect(page.getByText(restricted.name, { exact: true })).toBeVisible();
      await expect(page.getByText("2 series", { exact: true })).toBeVisible();
    } finally {
      await deleteReadingList(list.id);
      if (user) await deleteUsersByUsername(username);
      await Promise.all([visible, restricted].map((series: MetadataSeries) => removeMetadataSeries(series)));
    }
  });
});

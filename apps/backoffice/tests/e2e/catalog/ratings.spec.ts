import { test, expect } from "@playwright/test";

import { findUserByUsername, getSeriesRatings } from "../support/api";
import {
  createMetadataSeries,
  openMetadataSeries,
  removeMetadataSeries,
} from "../support/metadata";

/**
 * Notation d'une série par le lecteur actif (`e2e-reader`).
 *
 * L'interface envoie une note sur 10 (demi-étoiles) ; on clique la quatrième
 * étoile pleine, puis on vérifie la valeur persistée via l'API avec
 * `X-As-User`. La suppression de note est également exercée par l'UI.
 */

const READER_USERNAME = "e2e-reader";

test.describe("séries — notation", () => {
  test("note une série en demi-étoiles puis retire la note", async ({ page }) => {
    test.setTimeout(60_000);

    const reader = await findUserByUsername(READER_USERNAME);
    test.skip(!reader, `utilisateur ${READER_USERNAME} absent : lancer auth.setup.ts`);

    const series = await createMetadataSeries();

    try {
      await openMetadataSeries(page, series);

      const slider = page.getByRole("slider", { name: "Not rated" });
      await expect(slider).toBeVisible();

      // La quatrième étoile correspond à 8/10, soit 4,0 étoiles. Le clic est
      // volontairement placé sur la moitié droite de l'étoile : au centre
      // exact, la largeur impaire de l'étoile peut arrondir vers la demi-étoile.
      const fourthStar = slider.locator("span").nth(3);
      const starBox = await fourthStar.boundingBox();
      await fourthStar.click({
        position: { x: (starBox?.width ?? 20) - 2, y: (starBox?.height ?? 20) / 2 },
      });

      await expect(page.getByRole("slider", { name: "4 out of 5" })).toBeVisible();
      await expect(page.getByText("4.0", { exact: true })).toBeVisible();
      await expect
        .poll(async () => (await getSeriesRatings(series.seriesId, reader!.id)).user_rating)
        .toBe(8);

      // La note est bien rendue après rechargement (données serveur).
      await page.reload();
      await expect(page.getByRole("slider", { name: "4 out of 5" })).toBeVisible();

      // Retrait via le bouton dédié.
      await page.getByRole("button", { name: "Remove rating" }).click();
      await expect
        .poll(async () => (await getSeriesRatings(series.seriesId, reader!.id)).user_rating)
        .toBeNull();

      await page.reload();
      await expect(page.getByRole("slider", { name: "Not rated" })).toBeVisible();
    } finally {
      await removeMetadataSeries(series);
    }
  });
});

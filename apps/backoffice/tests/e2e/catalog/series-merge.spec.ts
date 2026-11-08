import { test, expect } from "@playwright/test";

import { escapeRegExp, uniqueName } from "../helpers";
import {
  createMetadataMatch,
  listMetadataLinksForSeries,
  seriesExists,
  type MetadataLinkSummary,
} from "../support/api";
import {
  createMetadataSeries,
  openMetadataSeries,
  removeMetadataSeries,
} from "../support/metadata";

/**
 * Fusion de deux séries de la bibliothèque temporaire.
 *
 * La série cible et la source sont créées par l'API, puis la fusion passe
 * entièrement par le menu d'actions et la modale. On vérifie ensuite que la
 * source a disparu, que la cible est conservée et que son lien de métadonnées
 * a bien été déplacé.
 */

test.describe("séries — fusion", () => {
  test("fusionne une série source dans la cible et déplace son lien de métadonnées", async ({
    page,
  }) => {
    test.setTimeout(60_000);

    const target = await createMetadataSeries();
    const source = await createMetadataSeries();
    const externalId = uniqueName("e2e-merge");

    try {
      // Lien de métadonnées attaché à la source : il doit suivre la fusion.
      await createMetadataMatch({
        library_id: source.libraryId,
        series_name: source.name,
        provider: "comicvine",
        external_id: externalId,
        title: source.name,
        metadata_json: {},
      });

      await openMetadataSeries(page, target);

      await page.getByRole("button", { name: "More", exact: true }).first().click();
      await page.getByRole("menuitem", { name: /Merge/ }).click();

      const mergePanel = page.getByTestId("modal-panel");
      await expect(mergePanel).toBeVisible();
      await mergePanel
        .getByPlaceholder("Search for a series...")
        .fill(source.name);

      const sourceResult = mergePanel.getByRole("button", {
        name: new RegExp(escapeRegExp(source.name)),
      });
      await expect(sourceResult).toBeVisible({ timeout: 10_000 });
      await sourceResult.click();

      await mergePanel.getByRole("button", { name: "Merge", exact: true }).click();
      await expect(mergePanel).toBeHidden();

      // La source est supprimée, la cible reste accessible.
      await expect.poll(() => seriesExists(source.seriesId)).toBe(false);
      await expect.poll(() => seriesExists(target.seriesId)).toBe(true);

      // Le lien de la source a été réaffecté à la cible.
      await expect
        .poll(async () => {
          const links: MetadataLinkSummary[] = await listMetadataLinksForSeries(target.seriesId);
          return links.map((link) => `${link.provider}:${link.external_id}`);
        })
        .toContain(`comicvine:${externalId}`);
    } finally {
      await removeMetadataSeries(source);
      await removeMetadataSeries(target);
    }
  });
});

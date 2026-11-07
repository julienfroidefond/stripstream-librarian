import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import { deleteUsersByUsername, findUserByUsername } from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

/**
 * Bascule d'utilisateur actif et restrictions de genres.
 *
 * L'utilisateur est créé via l'UI (les comptes créés par l'API n'apparaissent
 * pas immédiatement dans le UserSwitcher à cause du cache `revalidate: 60`).
 * La série de test porte un genre unique, bloqué pour ce lecteur : elle doit
 * disparaître de la liste tant qu'il est actif, puis réapparaître pour Admin.
 *
 * Les URLs de liste incluent un paramètre `e2e` unique : le cache de données
 * Next ne doit jamais resservir la réponse d'un autre utilisateur.
 */

test.describe("utilisateurs — restrictions de genres", () => {
  test("masque une série pour un lecteur restreint puis la réaffiche pour Admin", async ({
    page,
  }) => {
    const username = uniqueName("e2e-user");
    const genre = uniqueName("E2E Restricted Genre");
    const series = await createMetadataSeries({ genres: [genre] });

    try {
      // Création du lecteur via le formulaire des paramètres.
      await page.goto("/settings?tab=tokens");
      await expect(page.locator("main h1")).toContainText("Settings");

      await page.getByPlaceholder("Username").fill(username);
      await page.getByRole("button", { name: "Create", exact: true }).click();

      const userRow = page
        .locator("main table")
        .first()
        .getByRole("row")
        .filter({ hasText: username });
      await expect(userRow).toBeVisible();

      const user = await findUserByUsername(username);
      expect(user, "l'utilisateur créé doit exister côté API").not.toBeNull();

      // Blocage du genre unique via la modale de restrictions.
      await userRow.getByRole("button", { name: "Manage restrictions", exact: true }).click();
      const restrictionsPanel = page.getByTestId("modal-panel");
      await expect(restrictionsPanel).toBeVisible();

      const genreCheckbox = restrictionsPanel.getByRole("checkbox", {
        name: genre,
        exact: true,
      });
      await expect(genreCheckbox).toBeVisible();
      await genreCheckbox.check();
      await restrictionsPanel.getByRole("button", { name: "Enregistrer", exact: true }).click();
      await expect(restrictionsPanel).toBeHidden();

      // La restriction est affichée en badge dans la ligne du lecteur.
      await expect(userRow.getByText(genre, { exact: true })).toBeVisible();

      // Bascule vers le lecteur restreint.
      const header = page.getByRole("banner");
      await header.getByRole("button", { name: "e2e-reader", exact: true }).click();
      await header.getByRole("button", { name: username, exact: true }).click();
      await expect(header.getByRole("button", { name: username, exact: true })).toHaveCount(1);

      const readerQuery = encodeURIComponent(uniqueName("reader"));
      await page.goto(`/series?q=${encodeURIComponent(series.name)}&e2e=${readerQuery}`);
      await expect(page.locator("main")).toContainText("No series found matching your filters");
      await expect(page.getByTitle(series.name)).toHaveCount(0);

      // Retour à Admin : la série redevient visible.
      await header.getByRole("button", { name: username, exact: true }).click();
      await header.getByRole("button", { name: "Admin", exact: true }).click();
      await expect(header.getByRole("button", { name: "Admin", exact: true })).toHaveCount(1);

      const adminQuery = encodeURIComponent(uniqueName("admin"));
      await page.goto(`/series?q=${encodeURIComponent(series.name)}&e2e=${adminQuery}`);
      await expect(page.getByTitle(series.name)).toBeVisible();
    } finally {
      await deleteUsersByUsername(username);
      await removeMetadataSeries(series);
    }
  });
});

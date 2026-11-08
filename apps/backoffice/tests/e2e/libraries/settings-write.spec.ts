import { test, expect } from "@playwright/test";

import {
  findLibraryByName,
  updateLibraryMonitoring,
  updateLibraryProviders,
  updateLibraryTags,
} from "../support/api";
import { METADATA_LIBRARY_NAME } from "../support/metadata";

/**
 * Édition des réglages d'une bibliothèque depuis la modale de configuration.
 *
 * On utilise la bibliothèque temporaire `e2e-metadata` (jamais les données
 * réelles) et on restaure ses réglages via l'API dans le `finally`, même si
 * l'assertion échoue. Toutes les écritures testées passent par l'UI.
 */

test.describe("bibliothèques — réglages", () => {
  test("enregistre l'indexation, les providers et les tags puis les affiche", async ({ page }) => {
    test.setTimeout(60_000);

    const library = await findLibraryByName(METADATA_LIBRARY_NAME);
    test.skip(!library, `bibliothèque ${METADATA_LIBRARY_NAME} absente : lancer auth.setup.ts`);

    const original = {
      monitor_enabled: library!.monitor_enabled ?? false,
      scan_mode: library!.scan_mode ?? "manual",
      watcher_enabled: library!.watcher_enabled ?? false,
      metadata_refresh_mode: library!.metadata_refresh_mode ?? "manual",
      download_detection_mode: library!.download_detection_mode ?? "manual",
      metadata_provider: library!.metadata_provider ?? null,
      fallback_metadata_provider: library!.fallback_metadata_provider ?? null,
      tags: library!.tags ?? [],
    };

    const expected = {
      monitor: !original.monitor_enabled,
      watcher: !original.watcher_enabled,
      scanMode: original.scan_mode === "weekly" ? "daily" : "weekly",
      refreshMode: original.metadata_refresh_mode === "hourly" ? "daily" : "hourly",
      primaryProvider:
        original.metadata_provider === "comicvine" ? "google_books" : "comicvine",
      fallbackProvider: "open_library",
      tag: original.tags.includes("manga") ? "comics" : "manga",
    };

    // `fetchLibraries` est mis en cache 30 s côté Next. Auth setup recrée
    // `e2e-metadata` avec un nouvel id juste avant les tests : on force une
    // revalidation de la page (écriture no-op de valeurs identiques) pour que
    // la carte affichée pointe vers la bibliothèque courante et non l'ancienne.
    const revalidated = await page.request.patch(
      `/api/libraries/${library!.id}/metadata-provider`,
      {
        data: {
          metadata_provider: original.metadata_provider,
          fallback_metadata_provider: original.fallback_metadata_provider,
        },
      }
    );
    expect(revalidated.ok()).toBeTruthy();

    try {
      const card = page.locator("main .grid > div").filter({
        has: page.getByRole("heading", { name: METADATA_LIBRARY_NAME, exact: true }),
      });
      await expect(async () => {
        await page.goto(`/libraries?e2e=${encodeURIComponent(Date.now().toString(36))}`);
        await expect(card).toBeVisible();
        await expect(card.locator('input[name="id"]')).toHaveValue(library!.id);
      }).toPass({ timeout: 25_000 });

      await card.getByRole("button", { name: "Library settings" }).click();
      const panel = page.getByTestId("library-settings-panel");
      await expect(panel).toBeVisible();

      const monitor = panel.getByRole("checkbox", { name: "Scheduled scan" });
      if (expected.monitor) await monitor.check();
      else await monitor.uncheck();

      const watcher = panel.getByRole("checkbox", { name: "Real-time file watch" });
      if (expected.watcher) await watcher.check();
      else await watcher.uncheck();

      await panel.locator('select[name="scan_mode"]').selectOption(expected.scanMode);
      await panel
        .locator('select[name="metadata_refresh_mode"]')
        .selectOption(expected.refreshMode);

      const primarySelect = panel.locator('select[name="metadata_provider"]');
      await expect(primarySelect.locator(`option[value="${expected.primaryProvider}"]`)).toHaveCount(1);
      await primarySelect.selectOption(expected.primaryProvider);
      await panel
        .locator('select[name="fallback_metadata_provider"]')
        .selectOption(expected.fallbackProvider);

      await panel.getByRole("button", { name: expected.tag, exact: true }).click();

      const save = panel.getByRole("button", { name: "Save", exact: true });
      await save.click();
      await expect(panel).toBeHidden({ timeout: 15_000 });

      await expect
        .poll(async () => {
          const saved = await findLibraryByName(METADATA_LIBRARY_NAME);
          if (!saved) return null;
          return {
            monitor_enabled: saved.monitor_enabled,
            scan_mode: saved.scan_mode,
            watcher_enabled: saved.watcher_enabled,
            metadata_refresh_mode: saved.metadata_refresh_mode,
            metadata_provider: saved.metadata_provider,
            fallback_metadata_provider: saved.fallback_metadata_provider,
            tags: saved.tags,
          };
        })
        .toMatchObject({
          monitor_enabled: expected.monitor,
          scan_mode: expected.scanMode,
          watcher_enabled: expected.watcher,
          metadata_refresh_mode: expected.refreshMode,
          metadata_provider: expected.primaryProvider,
          fallback_metadata_provider: expected.fallbackProvider,
          tags: [...original.tags, expected.tag],
        });

      // La persistance est vérifiée par l'API : après `window.location.reload()`,
      // la carte peut encore afficher la liste mise en cache par Next (30 s),
      // comme pour les métadonnées de série revalidées par tag.
    } finally {
      // Restauration hors UI : le test ne doit pas laisser la bibliothèque
      // partagée dans un état différent pour les autres workers.
      await updateLibraryMonitoring(library!.id, {
        monitor_enabled: original.monitor_enabled,
        scan_mode: original.scan_mode,
        watcher_enabled: original.watcher_enabled,
        metadata_refresh_mode: original.metadata_refresh_mode,
        download_detection_mode: original.download_detection_mode,
      });
      await updateLibraryProviders(library!.id, {
        metadata_provider: original.metadata_provider,
        fallback_metadata_provider: original.fallback_metadata_provider,
      });
      await updateLibraryTags(library!.id, original.tags);
    }
  });
});

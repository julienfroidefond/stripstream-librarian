import { test, expect } from "@playwright/test";

/**
 * Actions de la page diagnostics : copie du rapport dans le presse-papiers et
 * sonde à la demande. La sonde est entièrement mockée : elle ne doit jamais
 * contacter les intégrations externes ni parcourir les vrais fichiers.
 */

test.describe("diagnostics — actions", () => {
  test("copie le rapport JSON dans le presse-papiers", async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto("/diagnostics");

    const diagnosticsResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" &&
        new URL(response.url()).pathname === "/api/diagnostics"
    );
    await page.getByRole("button", { name: "Copy diagnostics", exact: true }).click();
    const response = await diagnosticsResponse;
    expect(response.ok()).toBe(true);

    await expect(page.getByText("Diagnostics copied to clipboard", { exact: true })).toBeVisible();

    const clipboard = await page.evaluate(() => navigator.clipboard.readText());
    const payload = JSON.parse(clipboard) as {
      version?: string;
      database?: { migrations_applied?: number };
    };
    expect(payload.version).toBeTruthy();
    expect(typeof payload.database?.migrations_applied).toBe("number");
    expect(clipboard).toContain("\n");
  });

  test("lance la sonde mockée et affiche intégrations et fichiers manquants", async ({ page }) => {
    await page.route("**/api/diagnostics/probe", (route) =>
      route.fulfill({
        json: {
          sampled_at: "2026-01-02T03:04:05Z",
          integrations: [
            {
              id: "qbittorrent",
              configured: true,
              reachable: true,
              latency_ms: 12,
              detail: "v5.0.0",
            },
            {
              id: "prowlarr",
              configured: true,
              reachable: false,
              latency_ms: 5000,
              detail: "timeout",
            },
            {
              id: "telegram",
              configured: false,
              reachable: false,
              latency_ms: 0,
              detail: null,
            },
            {
              id: "ddl_alldebrid",
              configured: true,
              reachable: true,
              latency_ms: 34,
              detail: null,
            },
          ],
          missing_files: {
            checked: 42,
            missing: 2,
            samples: ["/e2e/library/missing-1.cbz", "/e2e/library/missing-2.cbz"],
          },
        },
      })
    );

    await page.goto("/diagnostics");
    await expect(
      page.getByText("No probe run since the page was opened.", { exact: true })
    ).toBeVisible();

    const probeResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" &&
        new URL(response.url()).pathname === "/api/diagnostics/probe"
    );
    await page.getByRole("button", { name: "Run probe", exact: true }).click();
    expect((await probeResponse).ok()).toBe(true);

    await expect(page.getByText(/Probe ran at/)).toBeVisible();
    const integrationRow = (name: string) =>
      page.getByText(name, { exact: true }).locator("xpath=ancestor::li[1]");
    await expect(
      integrationRow("qBittorrent").getByText("Reachable", { exact: true })
    ).toBeVisible();
    await expect(
      integrationRow("Prowlarr").getByText("Unreachable", { exact: true })
    ).toBeVisible();
    await expect(
      integrationRow("Telegram").getByText("Not configured", { exact: true })
    ).toBeVisible();
    await expect(
      integrationRow("DDL · Alldebrid").getByText("Reachable", { exact: true })
    ).toBeVisible();

    await expect(page.getByText("Files checked 42", { exact: true })).toBeVisible();
    await expect(page.getByText("Missing files 2", { exact: true })).toBeVisible();
    await expect(page.getByText("/e2e/library/missing-1.cbz", { exact: true })).toBeVisible();
    await expect(page.getByText("/e2e/library/missing-2.cbz", { exact: true })).toBeVisible();
  });

  test("affiche un toast si la sonde échoue", async ({ page }) => {
    await page.route("**/api/diagnostics/probe", (route) =>
      route.fulfill({ status: 500, json: { error: "probe unavailable" } })
    );

    await page.goto("/diagnostics");
    await page.getByRole("button", { name: "Run probe", exact: true }).click();
    await expect(page.getByText("Probe failed", { exact: true })).toBeVisible();
  });
});

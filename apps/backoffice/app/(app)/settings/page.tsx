import { getSettings, getCacheStats, getThumbnailStats, fetchUsers, apiFetch, fetchLibraries, LibraryDto, Settings } from "@/lib/api";
import { redirect } from "next/navigation";
import SettingsPage from "./SettingsPage";
import { TokensTab } from "./components/TokensTab";
import packageJson from "../../../package.json";

export const dynamic = "force-dynamic";

async function fetchIndexerVersion(): Promise<string> {
  const urls = [
    process.env.INDEXER_BASE_URL,
    "http://indexer:7081",
    "http://localhost:7081",
  ].filter(Boolean).map(u => u!.replace(/\/$/, ""));

  for (const url of urls) {
    try {
      const res = await fetch(`${url}/version`, { signal: AbortSignal.timeout(2000) });
      if (res.ok) {
        const data = await res.json();
        return data?.indexer ?? "?";
      }
    } catch { /* try next */ }
  }
  return "?";
}

export default async function SettingsPageWrapper({ searchParams }: { searchParams: Promise<{ tab?: string; created?: string }> }) {
  const { tab, created: createdToken } = await searchParams;
  // The reading overview moved out of Settings into its own top-level page.
  if (tab === "readingOverview") redirect("/progress");
  const [settings, cacheStats, thumbnailStats, users, statusMappings, seriesStatuses, providerStatuses, apiVersion, indexerVersion, libraries] = await Promise.all([
    // Defaults are merged by the settings draft provider; an unreachable API
    // still renders the page with the built-in defaults.
    getSettings().catch((): Settings => ({} as Settings)),
    getCacheStats().catch(() => ({ total_size_mb: 0, file_count: 0, directory: "/tmp/stripstream-image-cache", memory_size_mb: 0, memory_page_count: 0, memory_max_size_mb: 128 })),
    getThumbnailStats().catch(() => ({ total_size_mb: 0, file_count: 0, directory: "/data/thumbnails" })),
    fetchUsers().catch(() => []),
    apiFetch<Record<string, unknown>[]>("/settings/status-mappings").catch(() => []),
    apiFetch<unknown[]>("/series/statuses").catch(() => []),
    apiFetch<unknown[]>("/series/provider-statuses").catch(() => []),
    apiFetch<{ api?: string }>("/version").catch(() => ({ api: "?" })),
    fetchIndexerVersion(),
    fetchLibraries().catch(() => []),
  ]);

  // Tous les réglages `app_settings` arrivent déjà dans `getSettings()` : on
  // les passe en un seul snapshot au provider, qui possède le draft et les
  // écritures. Les mappings de statuts vivent dans leur propre table et gardent
  // leur endpoint dédié.
  const versions = {
    api: apiVersion?.api ?? "?",
    indexer: indexerVersion,
    backoffice: packageJson.version,
  };

  return (
    <SettingsPage
      initialSettings={settings}
      initialCacheStats={cacheStats}
      initialThumbnailStats={thumbnailStats}
      users={users}
      initialTab={tab}
      tokensContent={<TokensTab createdToken={createdToken} />}
      initialStatusMappings={statusMappings as Record<string, unknown>[]}
      initialSeriesStatuses={seriesStatuses as string[]}
      initialProviderStatuses={providerStatuses as string[]}
      initialLibraries={libraries as LibraryDto[]}
      versions={versions}
    />
  );
}

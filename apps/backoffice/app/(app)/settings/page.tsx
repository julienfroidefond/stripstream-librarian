import { getSettings, getCacheStats, getThumbnailStats, fetchUsers, apiFetch, fetchLibraries, LibraryDto, Settings } from "@/lib/api";
import { redirect } from "next/navigation";
import SettingsPage from "./SettingsPage";
import { TokensTab } from "./components/TokensTab";
import { readDownloadClientSetting } from "@/lib/downloadClient";
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
    getSettings().catch((): Settings => ({
      downloads_enabled: { enabled: false },
      image_processing: { format: "webp", quality: 85, filter: "lanczos3", max_width: 2160 },
      cache: { enabled: true, directory: "/tmp/stripstream-image-cache", max_size_mb: 10000, memory_max_size_mb: 128 },
      limits: { concurrent_renders: 4, timeout_seconds: 12, rate_limit_per_second: 120, concurrent_telegram_downloads: 2, telegram_download_connections: 4, concurrent_ddl_provider_downloads: 2, ddl_download_connections: 4, download_max_attempts: 5, download_stall_timeout_seconds: 180, download_timeout_minutes: 30, concurrent_alldebrid_downloads: 4, alldebrid_stall_timeout_minutes: 30 },
      thumbnail: { enabled: true, width: 300, height: 400, quality: 80, format: "webp", directory: "/data/thumbnails" },
      ai_tagging: { enabled: false, base_url: "https://openrouter.ai/api/v1", api_key: "", model: "openrouter/free", max_tags: 5 }
    })),
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
  // évite de refetcher chaque clé séparément. Les mappings de statuts vivent
  // dans leur propre table et gardent leur endpoint dédié.
  const prowlarr = settings.prowlarr ?? null;
  const qbittorrent = settings.qbittorrent ?? null;
  const alldebrid = settings.alldebrid ?? null;
  const telegram = settings.telegram ?? null;
  const anilist = settings.anilist ?? null;
  const komga = settings.komga ?? null;
  const metadataProviders = settings.metadata_providers ?? null;
  const renameFormat = settings.rename_format ?? null;
  const renameFormatHs = settings.rename_format_hs ?? null;
  const renameFormatInt = settings.rename_format_int ?? null;
  const renameFormatOneshot = settings.rename_format_oneshot ?? null;
  const downloadsEnabled = settings.downloads_enabled?.enabled === true;
  const downloadClientSetting = settings.download_client ?? null;

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
      initialProwlarr={prowlarr}
      initialQbittorrent={qbittorrent}
      initialAlldebrid={alldebrid}
      initialTelegram={telegram}
      initialAnilist={anilist}
      initialKomga={komga}
      initialMetadataProviders={metadataProviders}
      initialStatusMappings={statusMappings as Record<string, unknown>[]}
      initialSeriesStatuses={seriesStatuses as string[]}
      initialProviderStatuses={providerStatuses as string[]}
      initialRenameFormat={typeof renameFormat === "string" ? renameFormat : null}
      initialRenameFormatHs={typeof renameFormatHs === "string" ? renameFormatHs : null}
      initialRenameFormatInt={typeof renameFormatInt === "string" ? renameFormatInt : null}
      initialRenameFormatOneshot={typeof renameFormatOneshot === "string" ? renameFormatOneshot : null}
      initialDownloadsEnabled={downloadsEnabled}
      initialDownloadClient={readDownloadClientSetting(downloadClientSetting)}
      initialLibraries={libraries as LibraryDto[]}
      versions={versions}
    />
  );
}

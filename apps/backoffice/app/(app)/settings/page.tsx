import { getSettings, getCacheStats, getThumbnailStats, fetchUsers, apiFetch, fetchDownloadsEnabled, fetchLibraries, LibraryDto } from "@/lib/api";
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
  const [settings, cacheStats, thumbnailStats, users, prowlarr, qbittorrent, torrentImport, telegram, anilist, komga, metadataProviders, statusMappings, seriesStatuses, providerStatuses, apiVersion, indexerVersion, renameFormat, renameFormatHs, downloadsEnabled, libraries] = await Promise.all([
    getSettings().catch(() => ({
      image_processing: { format: "webp", quality: 85, filter: "lanczos3", max_width: 2160 },
      cache: { enabled: true, directory: "/tmp/stripstream-image-cache", max_size_mb: 10000, memory_max_size_mb: 128 },
      limits: { concurrent_renders: 4, timeout_seconds: 12, rate_limit_per_second: 120, concurrent_telegram_downloads: 2 },
      thumbnail: { enabled: true, width: 300, height: 400, quality: 80, format: "webp", directory: "/data/thumbnails" }
    })),
    getCacheStats().catch(() => ({ total_size_mb: 0, file_count: 0, directory: "/tmp/stripstream-image-cache", memory_size_mb: 0, memory_page_count: 0, memory_max_size_mb: 128 })),
    getThumbnailStats().catch(() => ({ total_size_mb: 0, file_count: 0, directory: "/data/thumbnails" })),
    fetchUsers().catch(() => []),
    apiFetch<Record<string, unknown>>("/settings/prowlarr").catch(() => null),
    apiFetch<Record<string, unknown>>("/settings/qbittorrent").catch(() => null),
    apiFetch<Record<string, unknown>>("/settings/torrent_import").catch(() => null),
    apiFetch<Record<string, unknown>>("/settings/telegram").catch(() => null),
    apiFetch<Record<string, unknown>>("/settings/anilist").catch(() => null),
    apiFetch<Record<string, unknown>>("/settings/komga").catch(() => null),
    apiFetch<Record<string, unknown>>("/settings/metadata_providers").catch(() => null),
    apiFetch<unknown[]>("/settings/status-mappings").catch(() => []),
    apiFetch<unknown[]>("/series/statuses").catch(() => []),
    apiFetch<unknown[]>("/series/provider-statuses").catch(() => []),
    apiFetch<{ api?: string }>("/version").catch(() => ({ api: "?" })),
    fetchIndexerVersion(),
    apiFetch<string>("/settings/rename_format").catch(() => null),
    apiFetch<string>("/settings/rename_format_hs").catch(() => null),
    fetchDownloadsEnabled(),
    fetchLibraries().catch(() => []),
  ]);

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
      initialTorrentImport={torrentImport}
      initialTelegram={telegram}
      initialAnilist={anilist}
      initialKomga={komga}
      initialMetadataProviders={metadataProviders}
      initialStatusMappings={statusMappings as Record<string, unknown>[]}
      initialSeriesStatuses={seriesStatuses as string[]}
      initialProviderStatuses={providerStatuses as string[]}
      initialRenameFormat={typeof renameFormat === "string" ? renameFormat : null}
      initialRenameFormatHs={typeof renameFormatHs === "string" ? renameFormatHs : null}
      initialDownloadsEnabled={downloadsEnabled}
      initialLibraries={libraries as LibraryDto[]}
      versions={versions}
    />
  );
}

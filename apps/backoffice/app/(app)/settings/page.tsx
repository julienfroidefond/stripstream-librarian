import { getSettings, getCacheStats, getThumbnailStats, fetchUsers, apiFetch } from "@/lib/api";
import SettingsPage from "./SettingsPage";
import packageJson from "../../../package.json";

export const dynamic = "force-dynamic";

async function fetchIndexerVersion(): Promise<string> {
  try {
    const indexerUrl = (process.env.INDEXER_BASE_URL || "http://indexer:7081").replace(/\/$/, "");
    const res = await fetch(`${indexerUrl}/version`, { signal: AbortSignal.timeout(3000) });
    if (res.ok) {
      const data = await res.json();
      return data?.indexer ?? "?";
    }
  } catch { /* ignore */ }
  return "?";
}

export default async function SettingsPageWrapper({ searchParams }: { searchParams: Promise<{ tab?: string }> }) {
  const { tab } = await searchParams;
  const [settings, cacheStats, thumbnailStats, users, prowlarr, qbittorrent, torrentImport, telegram, anilist, komga, metadataProviders, statusMappings, seriesStatuses, providerStatuses, apiVersion, indexerVersion] = await Promise.all([
    getSettings().catch(() => ({
      image_processing: { format: "webp", quality: 85, filter: "lanczos3", max_width: 2160 },
      cache: { enabled: true, directory: "/tmp/stripstream-image-cache", max_size_mb: 10000 },
      limits: { concurrent_renders: 4, timeout_seconds: 12, rate_limit_per_second: 120 },
      thumbnail: { enabled: true, width: 300, height: 400, quality: 80, format: "webp", directory: "/data/thumbnails" }
    })),
    getCacheStats().catch(() => ({ total_size_mb: 0, file_count: 0, directory: "/tmp/stripstream-image-cache" })),
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
      versions={versions}
    />
  );
}

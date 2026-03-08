import { getSettings, getCacheStats, getThumbnailStats } from "../../lib/api";
import SettingsPage from "./SettingsPage";

export const dynamic = "force-dynamic";

export default async function SettingsPageWrapper() {
  const settings = await getSettings().catch(() => ({
    image_processing: { format: "webp", quality: 85, filter: "lanczos3", max_width: 2160 },
    cache: { enabled: true, directory: "/tmp/stripstream-image-cache", max_size_mb: 10000 },
    limits: { concurrent_renders: 4, timeout_seconds: 12, rate_limit_per_second: 120 },
    thumbnail: { enabled: true, width: 300, height: 400, quality: 80, format: "webp", directory: "/data/thumbnails" }
  }));

  const cacheStats = await getCacheStats().catch(() => ({
    total_size_mb: 0,
    file_count: 0,
    directory: "/tmp/stripstream-image-cache"
  }));

  const thumbnailStats = await getThumbnailStats().catch(() => ({
    total_size_mb: 0,
    file_count: 0,
    directory: "/data/thumbnails"
  }));

  return <SettingsPage initialSettings={settings} initialCacheStats={cacheStats} initialThumbnailStats={thumbnailStats} />;
}

import { getSettings, getCacheStats } from "../../lib/api";
import SettingsPage from "./SettingsPage";

export const dynamic = "force-dynamic";

export default async function SettingsPageWrapper() {
  const settings = await getSettings().catch(() => ({
    image_processing: { format: "webp", quality: 85, filter: "lanczos3", max_width: 2160 },
    cache: { enabled: true, directory: "/tmp/stripstream-image-cache", max_size_mb: 10000 },
    limits: { concurrent_renders: 4, timeout_seconds: 12, rate_limit_per_second: 120 }
  }));

  const cacheStats = await getCacheStats().catch(() => ({
    total_size_mb: 0,
    file_count: 0,
    directory: "/tmp/stripstream-image-cache"
  }));

  return <SettingsPage initialSettings={settings} initialCacheStats={cacheStats} />;
}

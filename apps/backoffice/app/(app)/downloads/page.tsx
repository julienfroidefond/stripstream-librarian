import { redirect } from "next/navigation";
import { fetchTorrentDownloads, TorrentDownloadDto, LatestFoundPerLibraryDto, TelegramAvailableGroupDto, TelegramDownloadItemDto, DdlProviderDownloadDto, apiFetch, fetchDownloadsEnabled, fetchTelegramAvailable, fetchTelegramDownloads, fetchDdlProviderDownloads } from "@/lib/api";
import { DownloadsPage } from "./DownloadsPage";

export const dynamic = "force-dynamic";

async function isQbConfigured(): Promise<boolean> {
  try {
    const data = await apiFetch<{ url?: string; username?: string }>("/settings/qbittorrent");
    return !!(data && data.url?.trim() && data.username?.trim());
  } catch {
    return false;
  }
}

export default async function Page() {
  const downloadsEnabled = await fetchDownloadsEnabled();
  if (!downloadsEnabled) redirect("/");

  const [downloads, latestFound, qbConfigured, telegramAvailable, telegramDownloads, ddlProviderDownloads] = await Promise.all([
    fetchTorrentDownloads().catch(() => [] as TorrentDownloadDto[]),
    apiFetch<LatestFoundPerLibraryDto[]>("/download-detection/latest-found").catch(() => [] as LatestFoundPerLibraryDto[]),
    isQbConfigured(),
    fetchTelegramAvailable().catch(() => [] as TelegramAvailableGroupDto[]),
    fetchTelegramDownloads().catch(() => [] as TelegramDownloadItemDto[]),
    fetchDdlProviderDownloads().catch(() => [] as DdlProviderDownloadDto[]),
  ]);
  return (
    <DownloadsPage
      initialDownloads={downloads}
      initialLatestFound={latestFound}
      qbConfigured={qbConfigured}
      initialTelegramAvailable={telegramAvailable}
      initialTelegramDownloads={telegramDownloads}
      initialDdlProviderDownloads={ddlProviderDownloads}
    />
  );
}

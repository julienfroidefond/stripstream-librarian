import { redirect } from "next/navigation";
import { fetchTorrentDownloads, TorrentDownloadDto, LatestFoundPerLibraryDto, TelegramAvailableGroupDto, TelegramDownloadItemDto, DdlProviderDownloadDto, apiFetch, fetchTelegramAvailable, fetchTelegramDownloads, fetchDdlProviderDownloads } from "@/lib/api";
import { loadAcquisitionReadiness } from "@/lib/acquisition/readiness.server";
import { DownloadsPage } from "./DownloadsPage";
import { readDownloadClientSetting } from "@/lib/downloads/client";

export const dynamic = "force-dynamic";

export default async function Page() {
  const readiness = await loadAcquisitionReadiness();
  if (!readiness.downloads) redirect("/");

  const [downloads, latestFound, telegramAvailable, telegramDownloads, ddlProviderDownloads, downloadClientSetting] = await Promise.all([
    fetchTorrentDownloads().catch(() => [] as TorrentDownloadDto[]),
    apiFetch<LatestFoundPerLibraryDto[]>("/download-detection/latest-found").catch(() => [] as LatestFoundPerLibraryDto[]),
    fetchTelegramAvailable().catch(() => [] as TelegramAvailableGroupDto[]),
    fetchTelegramDownloads().catch(() => [] as TelegramDownloadItemDto[]),
    fetchDdlProviderDownloads().catch(() => [] as DdlProviderDownloadDto[]),
    apiFetch<unknown>("/settings/download_client").catch(() => null),
  ]);
  return (
    <DownloadsPage
      initialDownloads={downloads}
      initialLatestFound={latestFound}
      qbConfigured={readiness.qbittorrent}
      alldebridConfigured={readiness.alldebrid}
      initialDownloadClient={readDownloadClientSetting(downloadClientSetting)}
      initialTelegramAvailable={telegramAvailable}
      initialTelegramDownloads={telegramDownloads}
      initialDdlProviderDownloads={ddlProviderDownloads}
    />
  );
}

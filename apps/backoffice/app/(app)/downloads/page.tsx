import { redirect } from "next/navigation";
import { fetchTorrentDownloads, TorrentDownloadDto, LatestFoundPerLibraryDto, TelegramAvailableGroupDto, TelegramDownloadItemDto, DdlProviderDownloadDto, DownloadClient, apiFetch, fetchDownloadsEnabled, fetchTelegramAvailable, fetchTelegramDownloads, fetchDdlProviderDownloads } from "@/lib/api";
import { DownloadsPage } from "./DownloadsPage";
import { isClientConfigured, readDownloadClientSetting } from "@/lib/downloadClient";

export const dynamic = "force-dynamic";

/** Read a client's settings and decide whether it is usable. */
async function fetchClientConfigured(client: DownloadClient): Promise<boolean> {
  try {
    const data = await apiFetch<Record<string, unknown>>(`/settings/${client}`);
    return isClientConfigured(client, data);
  } catch {
    return false;
  }
}

export default async function Page() {
  const downloadsEnabled = await fetchDownloadsEnabled();
  if (!downloadsEnabled) redirect("/");

  const [downloads, latestFound, qbConfigured, alldebridConfigured, telegramAvailable, telegramDownloads, ddlProviderDownloads, downloadClientSetting] = await Promise.all([
    fetchTorrentDownloads().catch(() => [] as TorrentDownloadDto[]),
    apiFetch<LatestFoundPerLibraryDto[]>("/download-detection/latest-found").catch(() => [] as LatestFoundPerLibraryDto[]),
    fetchClientConfigured("qbittorrent"),
    fetchClientConfigured("alldebrid"),
    fetchTelegramAvailable().catch(() => [] as TelegramAvailableGroupDto[]),
    fetchTelegramDownloads().catch(() => [] as TelegramDownloadItemDto[]),
    fetchDdlProviderDownloads().catch(() => [] as DdlProviderDownloadDto[]),
    apiFetch<unknown>("/settings/download_client").catch(() => null),
  ]);
  return (
    <DownloadsPage
      initialDownloads={downloads}
      initialLatestFound={latestFound}
      qbConfigured={qbConfigured}
      alldebridConfigured={alldebridConfigured}
      initialDownloadClient={readDownloadClientSetting(downloadClientSetting)}
      initialTelegramAvailable={telegramAvailable}
      initialTelegramDownloads={telegramDownloads}
      initialDdlProviderDownloads={ddlProviderDownloads}
    />
  );
}

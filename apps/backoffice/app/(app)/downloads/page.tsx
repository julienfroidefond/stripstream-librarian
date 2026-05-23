import { redirect } from "next/navigation";
import { fetchTorrentDownloads, TorrentDownloadDto, LatestFoundPerLibraryDto, apiFetch, fetchDownloadsEnabled } from "@/lib/api";
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

  const [downloads, latestFound, qbConfigured] = await Promise.all([
    fetchTorrentDownloads().catch(() => [] as TorrentDownloadDto[]),
    apiFetch<LatestFoundPerLibraryDto[]>("/download-detection/latest-found").catch(() => [] as LatestFoundPerLibraryDto[]),
    isQbConfigured(),
  ]);
  return <DownloadsPage initialDownloads={downloads} initialLatestFound={latestFound} qbConfigured={qbConfigured} />;
}

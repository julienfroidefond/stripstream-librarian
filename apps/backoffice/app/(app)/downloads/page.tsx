import { fetchTorrentDownloads, TorrentDownloadDto, LatestFoundPerLibraryDto, apiFetch } from "@/lib/api";
import { DownloadsPage } from "./DownloadsPage";

export const dynamic = "force-dynamic";

export default async function Page() {
  const [downloads, latestFound] = await Promise.all([
    fetchTorrentDownloads().catch(() => [] as TorrentDownloadDto[]),
    apiFetch<LatestFoundPerLibraryDto[]>("/download-detection/latest-found").catch(() => [] as LatestFoundPerLibraryDto[]),
  ]);
  return <DownloadsPage initialDownloads={downloads} initialLatestFound={latestFound} />;
}

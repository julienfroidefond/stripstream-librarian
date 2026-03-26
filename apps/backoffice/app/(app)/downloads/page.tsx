import { fetchTorrentDownloads, TorrentDownloadDto } from "@/lib/api";
import { DownloadsPage } from "./DownloadsPage";

export const dynamic = "force-dynamic";

export default async function Page() {
  const downloads = await fetchTorrentDownloads().catch(() => [] as TorrentDownloadDto[]);
  return <DownloadsPage initialDownloads={downloads} />;
}

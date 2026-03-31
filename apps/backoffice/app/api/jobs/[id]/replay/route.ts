import { NextRequest, NextResponse } from "next/server";
import { apiFetch, IndexJobDto, rebuildIndex, rebuildThumbnails, regenerateThumbnails, startMetadataBatch, startMetadataRefresh, startMetadataRefreshAll, startReadingStatusMatch, startReadingStatusPush, startDownloadDetection } from "@/lib/api";

export async function POST(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const job = await apiFetch<IndexJobDto>(`/index/jobs/${id}`);
    const libraryId = job.library_id ?? undefined;

    switch (job.type) {
      case "rebuild":
        return NextResponse.json(await rebuildIndex(libraryId));
      case "full_rebuild":
        return NextResponse.json(await rebuildIndex(libraryId, true));
      case "rescan":
        return NextResponse.json(await rebuildIndex(libraryId, false, true));
      case "scan":
        return NextResponse.json(await rebuildIndex(libraryId));
      case "thumbnail_rebuild":
        return NextResponse.json(await rebuildThumbnails(libraryId));
      case "thumbnail_regenerate":
        return NextResponse.json(await regenerateThumbnails(libraryId));
      case "metadata_batch":
        if (!libraryId) return NextResponse.json({ error: "Library ID required for metadata batch" }, { status: 400 });
        return NextResponse.json(await startMetadataBatch(libraryId));
      case "metadata_refresh":
        if (!libraryId) return NextResponse.json({ error: "Library ID required for metadata refresh" }, { status: 400 });
        return NextResponse.json(await startMetadataRefresh(libraryId));
      case "metadata_refresh_all":
        if (!libraryId) return NextResponse.json({ error: "Library ID required for metadata refresh all" }, { status: 400 });
        return NextResponse.json(await startMetadataRefreshAll(libraryId));
      case "reading_status_match":
        if (!libraryId) return NextResponse.json({ error: "Library ID required for reading status match" }, { status: 400 });
        return NextResponse.json(await startReadingStatusMatch(libraryId));
      case "reading_status_push":
        if (!libraryId) return NextResponse.json({ error: "Library ID required for reading status push" }, { status: 400 });
        return NextResponse.json(await startReadingStatusPush(libraryId));
      case "download_detection":
        if (!libraryId) return NextResponse.json({ error: "Library ID required for download detection" }, { status: 400 });
        return NextResponse.json(await startDownloadDetection(libraryId));
      default:
        return NextResponse.json({ error: `Cannot replay job type: ${job.type}` }, { status: 400 });
    }
  } catch (error) {
    return NextResponse.json({ error: "Failed to replay job" }, { status: 500 });
  }
}

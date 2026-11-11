import { NextRequest, NextResponse } from "next/server";
import { apiFetch, IndexJobDto, listJobTypes, rebuildIndex, rebuildThumbnails, regenerateThumbnails, startMetadataBatch, startMetadataRefresh, startMetadataRefreshAll, startReadingStatusMatch, startReadingStatusPush, startReadingStatusPull, startDownloadDetection, startRssPoll, startDdlSearchAll } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const [job, jobTypeCatalog] = await Promise.all([
    apiFetch<IndexJobDto>(`/index/jobs/${id}`),
    listJobTypes(),
  ]);
  const catalogEntry = jobTypeCatalog.find((entry) => entry.job_type === job.type);
  if (!catalogEntry?.replayable) {
    return NextResponse.json({ error: `Cannot replay job type: ${job.type}` }, { status: 400 });
  }

  const libraryId = job.library_id ?? undefined;
  if (catalogEntry.requires_library && !libraryId) {
    return NextResponse.json({ error: `Library ID required for ${job.type}` }, { status: 400 });
  }

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
      return NextResponse.json(await startMetadataBatch(libraryId));
    case "metadata_batch_rematch":
      return NextResponse.json(await startMetadataBatch(libraryId, true));
    case "metadata_refresh":
      return NextResponse.json(await startMetadataRefresh(libraryId));
    case "metadata_refresh_all":
      return NextResponse.json(await startMetadataRefreshAll(libraryId));
    case "reading_status_match":
      return NextResponse.json(await startReadingStatusMatch(libraryId));
    case "reading_status_push":
      return NextResponse.json(await startReadingStatusPush(libraryId));
    case "rating_pull":
      return NextResponse.json(await startReadingStatusPull());
    case "download_detection":
      return NextResponse.json(await startDownloadDetection(libraryId));
    case "prowlarr_rss":
      return NextResponse.json(await startRssPoll(libraryId));
    case "ddl_search":
      return NextResponse.json(await startDdlSearchAll());
    default:
      return NextResponse.json({ error: `Cannot replay job type: ${job.type}` }, { status: 400 });
  }
}, { message: "Failed to replay job" });

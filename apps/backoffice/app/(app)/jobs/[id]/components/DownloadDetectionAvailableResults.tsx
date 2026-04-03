"use client";

import type { DownloadDetectionResultDto } from "@/lib/api";
import { AvailableDownloadsSection } from "@/app/(app)/downloads/DownloadsPage";

export function DownloadDetectionAvailableResults({
  results,
  libraryId,
  libraryName,
}: {
  results: DownloadDetectionResultDto[];
  libraryId: string;
  libraryName: string;
}) {
  return (
    <AvailableDownloadsSection
      latestFound={[{
        library_id: libraryId,
        library_name: libraryName,
        results: results.map(r => ({
          id: r.id,
          series_id: r.series_id || r.id,
          series_name: r.series_name,
          missing_count: r.missing_count,
          available_releases: r.available_releases,
          updated_at: new Date().toISOString(),
          failed_download_count: 0,
        })),
      }]}
      onDeleted={() => {}}
    />
  );
}

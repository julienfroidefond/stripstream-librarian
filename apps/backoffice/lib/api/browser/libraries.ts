/** Browser-side library settings and folder browsing. */

import type { FolderItem, LibraryDto } from "../libraries";
import { browserFetch, withQuery } from "./transport";

export interface LibraryMonitoringUpdate {
  monitor_enabled: boolean;
  scan_mode: string;
  watcher_enabled?: boolean;
  metadata_refresh_mode?: string;
  download_detection_mode?: string;
}

export const librariesApi = {
  updateMonitoring(libraryId: string, body: LibraryMonitoringUpdate): Promise<LibraryDto> {
    return browserFetch<LibraryDto>(
      `/api/libraries/${encodeURIComponent(libraryId)}/monitoring`,
      { method: "PATCH", body }
    );
  },

  updateMetadataProvider(
    libraryId: string,
    provider: string | null,
    fallbackProvider: string | null
  ): Promise<LibraryDto> {
    return browserFetch<LibraryDto>(
      `/api/libraries/${encodeURIComponent(libraryId)}/metadata-provider`,
      {
        method: "PATCH",
        body: {
          metadata_provider: provider,
          fallback_metadata_provider: fallbackProvider,
        },
      }
    );
  },

  updateReadingStatusProvider(
    libraryId: string,
    provider: string | null,
    pushMode: string
  ): Promise<LibraryDto> {
    return browserFetch<LibraryDto>(
      `/api/libraries/${encodeURIComponent(libraryId)}/reading-status-provider`,
      {
        method: "PATCH",
        body: {
          reading_status_provider: provider,
          reading_status_push_mode: pushMode,
        },
      }
    );
  },

  updateTags(libraryId: string, tags: string[]): Promise<LibraryDto> {
    return browserFetch<LibraryDto>(`/api/libraries/${encodeURIComponent(libraryId)}/tags`, {
      method: "PATCH",
      body: { tags },
    });
  },

  folders(path?: string): Promise<FolderItem[]> {
    return browserFetch<FolderItem[]>(withQuery("/api/folders", { path }));
  },
};

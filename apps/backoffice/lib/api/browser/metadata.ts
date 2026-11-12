/** Browser-side metadata provider search, matching and link management. */

import type {
  ExternalMetadataLinkDto,
  MetadataProviderDto,
  MissingBooksDto,
  SeriesCandidateDto,
  SyncReport,
} from "../metadata";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export interface MetadataSearchInput {
  library_id: string;
  series_name: string;
  provider?: string;
}

export interface MetadataMatchInput {
  library_id: string;
  series_name: string;
  provider: string;
  external_id: string;
  external_url: string | null;
  confidence?: number | null;
  title: string;
  metadata_json: Record<string, unknown>;
  total_volumes?: number | null;
}

export interface MetadataApproveInput {
  id: string;
  sync_series: boolean;
  sync_books: boolean;
}

export interface MetadataApproveResult {
  status: string;
  books_synced: number;
  report?: SyncReport | null;
}

export interface MetadataLinkUpdate {
  is_primary?: boolean;
  sync_series?: boolean;
  sync_books?: boolean;
}

export const metadataApi = {
  providers(): Promise<MetadataProviderDto[]> {
    return browserFetch<MetadataProviderDto[]>("/api/metadata/providers");
  },

  search(input: MetadataSearchInput): Promise<SeriesCandidateDto[]> {
    return browserFetch<SeriesCandidateDto[]>("/api/metadata/search", {
      method: "POST",
      body: input,
    });
  },

  match(input: MetadataMatchInput): Promise<ExternalMetadataLinkDto> {
    return browserFetch<ExternalMetadataLinkDto>("/api/metadata/match", {
      method: "POST",
      body: input,
    });
  },

  approve(input: MetadataApproveInput): Promise<MetadataApproveResult> {
    return browserFetch<MetadataApproveResult>("/api/metadata/approve", {
      method: "POST",
      body: input,
    });
  },

  missing(linkId: string): Promise<MissingBooksDto> {
    return browserFetch<MissingBooksDto>(withQuery("/api/metadata/missing", { id: linkId }));
  },

  updateLink(
    linkId: string,
    body: MetadataLinkUpdate
  ): Promise<{ link: ExternalMetadataLinkDto; report: unknown }> {
    return browserFetch<{ link: ExternalMetadataLinkDto; report: unknown }>(
      withQuery("/api/metadata/links", { id: linkId }),
      {
        method: "PATCH",
        body,
      }
    );
  },

  deleteLink(linkId: string): Promise<{ deleted: boolean }> {
    return browserFetch<{ deleted: boolean }>(withQuery("/api/metadata/links", { id: linkId }), {
      method: "DELETE",
    });
  },

  refreshLink(linkId: string): Promise<void> {
    return browserFetchVoid(`/api/metadata/refresh-link/${encodeURIComponent(linkId)}`, {
      method: "POST",
    });
  },
};

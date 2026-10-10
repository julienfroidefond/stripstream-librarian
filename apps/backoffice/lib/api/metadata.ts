/**
 * Métadonnées externes : liens fournisseurs, matching, livres manquants.
 */

import { apiFetch } from "./client";
import type { AnilistSeriesLinkDto } from "./anilist";

export type MetadataProviderDto = {
  id: string;
  label: string;
  requires_api_key: boolean;
};

export type SeriesCandidateDto = {
  provider: string;
  external_id: string;
  title: string;
  authors: string[];
  description: string | null;
  publishers: string[];
  start_year: number | null;
  total_volumes: number | null;
  cover_url: string | null;
  external_url: string | null;
  confidence: number;
  metadata_json: Record<string, unknown>;
};

export type ExternalMetadataLinkDto = {
  id: string;
  library_id: string;
  series_name: string;
  provider: string;
  external_id: string;
  external_url: string | null;
  status: string;
  is_primary: boolean;
  confidence: number | null;
  metadata_json: Record<string, unknown>;
  total_volumes_external: number | null;
  matched_at: string;
  approved_at: string | null;
  synced_at: string | null;
};

export type FieldChange = {
  field: string;
  old_value?: unknown;
  new_value?: unknown;
};

export type SeriesSyncReport = {
  fields_updated: FieldChange[];
  fields_skipped: FieldChange[];
};

export type BookSyncReport = {
  book_id: string;
  title: string;
  volume: number | null;
  fields_updated: FieldChange[];
  fields_skipped: FieldChange[];
};

export type SyncReport = {
  series: SeriesSyncReport | null;
  books: BookSyncReport[];
  books_matched: number;
  books_unmatched: number;
  books_message?: string;
};

export type MissingBooksDto = {
  total_external: number;
  total_local: number;
  missing_count: number;
  missing_books: {
    title: string | null;
    volume_number: number | null;
    external_book_id: string | null;
    cover_url: string | null;
  }[];
};

export async function searchMetadata(libraryId: string, seriesName: string, provider?: string) {
  return apiFetch<SeriesCandidateDto[]>("/metadata/search", {
    method: "POST",
    body: JSON.stringify({ library_id: libraryId, series_name: seriesName, provider: provider || undefined }),
  });
}

export async function fetchMetadataProviders() {
  return apiFetch<MetadataProviderDto[]>("/metadata/providers");
}

export async function createMetadataMatch(data: {
  library_id: string;
  series_name: string;
  provider: string;
  external_id: string;
  external_url?: string | null;
  confidence?: number | null;
  title: string;
  metadata_json: Record<string, unknown>;
  total_volumes?: number | null;
}) {
  return apiFetch<ExternalMetadataLinkDto>("/metadata/match", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function approveMetadataMatch(id: string, syncSeries: boolean, syncBooks: boolean, isPrimary?: boolean) {
  return apiFetch<{ status: string; report: SyncReport }>(`/metadata/approve/${id}`, {
    method: "POST",
    body: JSON.stringify({ sync_series: syncSeries, sync_books: syncBooks, is_primary: isPrimary }),
  });
}

export async function rejectMetadataMatch(id: string) {
  return apiFetch<{ status: string }>(`/metadata/reject/${id}`, {
    method: "POST",
  });
}

export async function getMetadataLink(seriesId: string) {
  const params = new URLSearchParams();
  params.set("series_id", seriesId);
  return apiFetch<ExternalMetadataLinkDto[]>(`/metadata/links?${params.toString()}`, {
    next: { revalidate: 10, tags: ["metadata", `series:${seriesId}`] },
  });
}

export async function getReadingStatusLink(seriesId: string) {
  return apiFetch<AnilistSeriesLinkDto>(`/series/${seriesId}/anilist`, {
    next: { revalidate: 60, tags: ["series", `series:${seriesId}`] },
  });
}

export async function getMissingBooks(linkId: string) {
  return apiFetch<MissingBooksDto>(`/metadata/missing/${linkId}`, {
    next: { revalidate: 10, tags: ["metadata", `link:${linkId}`] },
  });
}

export async function deleteMetadataLink(id: string) {
  return apiFetch<{ deleted: boolean }>(`/metadata/links/${id}`, {
    method: "DELETE",
  });
}

export async function setMetadataLinkPrimary(
  id: string,
  isPrimary: boolean,
  syncSeries = true,
  syncBooks = true,
) {
  return apiFetch<{ link: ExternalMetadataLinkDto; report: SyncReport }>(`/metadata/links/${id}`, {
    method: "PATCH",
    body: JSON.stringify({ is_primary: isPrimary, sync_series: syncSeries, sync_books: syncBooks }),
  });
}

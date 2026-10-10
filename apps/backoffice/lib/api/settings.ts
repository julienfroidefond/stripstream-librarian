/**
 * Paramètres applicatifs, cache, mappings de statuts.
 */

import { apiFetch } from "./client";

export type Settings = {
  ai_tagging?: {
    enabled: boolean;
    base_url: string;
    api_key: string;
    model: string;
    max_tags: number;
    prompt?: string;
  };
  image_processing: {
    format: string;
    quality: number;
    filter: string;
    max_width: number;
  };
  cache: {
    enabled: boolean;
    directory: string;
    max_size_mb: number;
    memory_max_size_mb: number;
  };
  limits: {
    concurrent_renders: number;
    timeout_seconds: number;
    rate_limit_per_second: number;
    concurrent_telegram_downloads: number;
  };
  thumbnail: {
    enabled: boolean;
    width: number;
    height: number;
    quality: number;
    format: string;
    directory: string;
  };
};

export type CacheStats = {
  total_size_mb: number;
  file_count: number;
  directory: string;
  memory_size_mb: number;
  memory_page_count: number;
  memory_max_size_mb: number;
};

export type ClearCacheResponse = {
  success: boolean;
  message: string;
};

export type ThumbnailStats = {
  total_size_mb: number;
  file_count: number;
  directory: string;
};

export async function getSettings() {
  return apiFetch<Settings>("/settings", { cache: "no-store" });
}

export async function fetchDownloadsEnabled(): Promise<boolean> {
  return apiFetch<{ enabled: boolean }>("/settings/downloads_enabled", {
    next: { revalidate: 300 },
  })
    .then(d => d?.enabled === true)
    .catch(() => false);
}

export async function updateSetting(key: string, value: unknown) {
  return apiFetch<unknown>(`/settings/${key}`, {
    method: "POST",
    body: JSON.stringify({ value }),
  });
}

export async function getCacheStats() {
  return apiFetch<CacheStats>("/settings/cache/stats", { cache: "no-store" });
}

export async function clearCache() {
  return apiFetch<ClearCacheResponse>("/settings/cache/clear", {
    method: "POST",
  });
}

export async function getThumbnailStats() {
  return apiFetch<ThumbnailStats>("/settings/thumbnail/stats", { cache: "no-store" });
}

export type StatusMappingDto = {
  id: string;
  provider_status: string;
  mapped_status: string | null;
};

export async function fetchStatusMappings(): Promise<StatusMappingDto[]> {
  return apiFetch<StatusMappingDto[]>("/settings/status-mappings", { next: { revalidate: 60 } });
}

export async function upsertStatusMapping(provider_status: string, mapped_status: string): Promise<StatusMappingDto> {
  return apiFetch<StatusMappingDto>("/settings/status-mappings", {
    method: "POST",
    body: JSON.stringify({ provider_status, mapped_status }),
  });
}

export async function deleteStatusMapping(id: string): Promise<void> {
  await apiFetch<unknown>(`/settings/status-mappings/${id}`, { method: "DELETE" });
}

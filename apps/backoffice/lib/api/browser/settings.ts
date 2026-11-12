/**
 * Browser-side settings, cache and status-mapping calls.
 *
 * Section writes replace the whole section (`{ value }`), matching the proxy
 * and the Rust API. Card forms go through the settings draft; these functions
 * are also used by the draft writer and by resources with their own endpoints.
 */

import type { CacheStats, ClearCacheResponse, StatusMappingDto } from "../settings";
import { browserFetch } from "./transport";

export type TestConnectionTarget = "prowlarr" | "qbittorrent" | "alldebrid" | "telegram";

export interface TestConnectionResult {
  success: boolean;
  message: string;
  error?: string;
}

const TEST_ENDPOINTS: Record<TestConnectionTarget, string> = {
  prowlarr: "/api/prowlarr/test",
  qbittorrent: "/api/qbittorrent/test",
  alldebrid: "/api/alldebrid/test",
  telegram: "/api/telegram/test",
};

export interface AiTaggingTestInput {
  base_url: string;
  api_key: string;
  model: string;
}

export const settingsApi = {
  /** Read one stored `app_settings` section (null when absent). */
  section<T = Record<string, unknown>>(key: string): Promise<T | null> {
    return browserFetch<T | null>(`/api/settings/${encodeURIComponent(key)}`);
  },

  /** Replace one `app_settings` section. */
  saveSection<T = unknown>(key: string, value: unknown): Promise<T> {
    return browserFetch<T>(`/api/settings/${encodeURIComponent(key)}`, {
      method: "POST",
      body: { value },
    });
  },

  cacheStats(): Promise<CacheStats> {
    return browserFetch<CacheStats>("/api/settings/cache/stats", { cache: "no-store" });
  },

  clearCache(): Promise<ClearCacheResponse & { error?: string }> {
    return browserFetch<ClearCacheResponse & { error?: string }>("/api/settings/cache/clear", {
      method: "POST",
    });
  },

  testAiTagging(input: AiTaggingTestInput): Promise<{ success: boolean; message: string; error?: string }> {
    return browserFetch("/api/settings/ai_tagging/test", { method: "POST", body: input });
  },

  testConnection(target: TestConnectionTarget): Promise<TestConnectionResult> {
    return browserFetch<TestConnectionResult>(TEST_ENDPOINTS[target]);
  },

  createStatusMapping(providerStatus: string, mappedStatus: string): Promise<StatusMappingDto> {
    return browserFetch<StatusMappingDto>("/api/settings/status-mappings", {
      method: "POST",
      body: { provider_status: providerStatus, mapped_status: mappedStatus },
    });
  },

  deleteStatusMapping(id: string): Promise<StatusMappingDto> {
    return browserFetch<StatusMappingDto>(`/api/settings/status-mappings/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  },
};

/** Browser-side Telegram monitor configuration and library. */

import type {
  TelegramAvailableGroupDto,
  TelegramBookLinkDto,
  TelegramDownloadItemDto,
  TelegramMonitorStatus,
  TelegramSearchResultDto,
  TelegramSourceDto,
} from "../telegram";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export interface TelegramMonitorSettings {
  api_id: number;
  api_hash: string;
  phone: string;
  sync_interval_minutes?: number;
}

export interface TelegramChannelSuggestion {
  username: string | null;
  title: string;
  kind: string;
}

export const telegramApi = {
  status(): Promise<TelegramMonitorStatus> {
    return browserFetch<TelegramMonitorStatus>("/api/telegram-monitor/status");
  },

  saveSettings(settings: TelegramMonitorSettings): Promise<void> {
    return browserFetchVoid("/api/telegram-monitor/settings", {
      method: "POST",
      body: settings,
    });
  },

  /** Update only the background sync cadence, keeping the stored credentials. */
  saveSyncInterval(minutes: number): Promise<void> {
    return browserFetchVoid("/api/telegram-monitor/settings", {
      method: "POST",
      body: { sync_interval_minutes: minutes },
    });
  },

  startAuth(): Promise<void> {
    return browserFetchVoid("/api/telegram-monitor/auth/start", { method: "POST" });
  },

  verifyCode(code: string): Promise<void> {
    return browserFetchVoid("/api/telegram-monitor/auth/verify", {
      method: "POST",
      body: { code },
    });
  },

  disconnect(): Promise<void> {
    return browserFetchVoid("/api/telegram-monitor/auth", { method: "DELETE" });
  },

  channels(q: string): Promise<TelegramChannelSuggestion[]> {
    return browserFetch<TelegramChannelSuggestion[]>(
      withQuery("/api/telegram-monitor/channels", { q })
    );
  },

  sources(): Promise<TelegramSourceDto[]> {
    return browserFetch<TelegramSourceDto[]>("/api/telegram-monitor/sources");
  },

  addSource(data: {
    channel_username: string;
    library_id?: string | null;
  }): Promise<TelegramSourceDto> {
    return browserFetch<TelegramSourceDto>("/api/telegram-monitor/sources", {
      method: "POST",
      body: data,
    });
  },

  deleteSource(id: string): Promise<void> {
    return browserFetchVoid(`/api/telegram-monitor/sources/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  },

  sync(): Promise<{ synced: number; new_books: number }> {
    return browserFetch("/api/telegram-monitor/sync", { method: "POST" });
  },

  startSync(): Promise<{ id: string | null; status: string }> {
    return browserFetch("/api/telegram-monitor/start", { method: "POST" });
  },

  startSyncIncremental(): Promise<{ id: string | null; status: string }> {
    return browserFetch("/api/telegram-monitor/start-incremental", { method: "POST" });
  },

  books(): Promise<TelegramBookLinkDto[]> {
    return browserFetch<TelegramBookLinkDto[]>("/api/telegram-monitor/books");
  },

  /** Search the connected Telegram account for importable books. */
  liveSearch(query: string): Promise<TelegramSearchResultDto[]> {
    return browserFetch<TelegramSearchResultDto[]>("/api/telegram-monitor/live-search", {
      method: "POST",
      body: { query },
    });
  },

  downloadBook(id: string, libraryId?: string | null): Promise<void> {
    return browserFetchVoid(
      `/api/telegram-monitor/books/${encodeURIComponent(id)}/download`,
      {
        method: "POST",
        body: libraryId === undefined ? {} : { library_id: libraryId },
      }
    );
  },

  /** Dismiss a discovered book, or permanently remove it with `hard`. */
  deleteBook(id: string, hard = false): Promise<void> {
    return browserFetchVoid(
      withQuery(`/api/telegram-monitor/books/${encodeURIComponent(id)}`, {
        hard: hard ? "true" : undefined,
      }),
      { method: "DELETE" }
    );
  },

  available(): Promise<TelegramAvailableGroupDto[]> {
    return browserFetch<TelegramAvailableGroupDto[]>("/api/telegram-monitor/available");
  },

  downloads(): Promise<TelegramDownloadItemDto[]> {
    return browserFetch<TelegramDownloadItemDto[]>("/api/telegram-monitor/downloads");
  },
};

/**
 * Moniteur Telegram.
 */

import { apiFetch } from "./client";

export type TelegramMonitorStatus = {
  sync_interval_minutes: number;
  configured: boolean;
  authorized: boolean;
  phone: string | null;
  api_id: number | null;
};

export type TelegramSourceDto = {
  id: string;
  channel_username: string;
  channel_title: string | null;
  library_id: string | null;
  enabled: boolean;
  created_at: string;
};

export type TelegramBookLinkDto = {
  id: string;
  source_id: string;
  channel_username: string;
  message_id: number;
  filename: string;
  file_size: number | null;
  mime_type: string | null;
  message_text: string | null;
  status: "available" | "downloading" | "imported" | "failed" | "dismissed";
  library_id: string | null;
  book_id: string | null;
  error_message: string | null;
  series_name: string | null;
  volume_number: number | null;
  created_at: string;
};

export type TelegramAvailableBookDto = {
  id: string;
  channel_username: string;
  message_id: number;
  filename: string;
  file_size: number | null;
  volume_number: number | null;
  status: string;
  created_at: string;
};

export type TelegramAvailableGroupDto = {
  series_name: string;
  series_id: string | null;
  library_id: string;
  library_name: string;
  owned_volumes: number[];
  series_missing_count: number;
  books: TelegramAvailableBookDto[];
};

export type TelegramDownloadItemDto = {
  id: string;
  series_name: string | null;
  series_id: string | null;
  library_id: string | null;
  library_name: string | null;
  channel_username: string;
  filename: string;
  file_size: number | null;
  bytes_downloaded: number;
  volume_number: number | null;
  status: "queued" | "downloading" | "imported" | "failed";
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export async function getTelegramMonitorStatus(): Promise<TelegramMonitorStatus> {
  return apiFetch<TelegramMonitorStatus>("/telegram-monitor/status");
}

export async function saveTelegramMonitorSettings(data: {
  api_id: number;
  api_hash: string;
  phone: string;
}): Promise<void> {
  await apiFetch("/telegram-monitor/settings", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function startTelegramAuth(): Promise<void> {
  await apiFetch("/telegram-monitor/auth/start", { method: "POST" });
}

export async function verifyTelegramCode(code: string): Promise<void> {
  await apiFetch("/telegram-monitor/auth/verify", {
    method: "POST",
    body: JSON.stringify({ code }),
  });
}

export async function disconnectTelegram(): Promise<void> {
  await apiFetch("/telegram-monitor/auth", { method: "DELETE" });
}

export async function listTelegramSources(): Promise<TelegramSourceDto[]> {
  return apiFetch<TelegramSourceDto[]>("/telegram-monitor/sources");
}

export async function addTelegramSource(data: {
  channel_username: string;
  library_id?: string;
}): Promise<TelegramSourceDto> {
  return apiFetch<TelegramSourceDto>("/telegram-monitor/sources", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function deleteTelegramSource(id: string): Promise<void> {
  await apiFetch(`/telegram-monitor/sources/${id}`, { method: "DELETE" });
}

export async function syncTelegramSources(): Promise<{ synced: number; new_books: number }> {
  return apiFetch<{ synced: number; new_books: number }>("/telegram-monitor/sync", {
    method: "POST",
  });
}

export async function startTelegramSync(): Promise<{ id: string | null; status: string }> {
  return apiFetch<{ id: string | null; status: string }>("/telegram-monitor/start", {
    method: "POST",
  });
}

export async function startTelegramSyncIncremental(): Promise<{ id: string | null; status: string }> {
  return apiFetch<{ id: string | null; status: string }>("/telegram-monitor/start-incremental", {
    method: "POST",
  });
}

export async function fetchTelegramAuthorized(): Promise<boolean> {
  try {
    const status = await getTelegramMonitorStatus();
    return status.authorized;
  } catch {
    return false;
  }
}

export async function listTelegramBooks(): Promise<TelegramBookLinkDto[]> {
  return apiFetch<TelegramBookLinkDto[]>("/telegram-monitor/books");
}

export async function downloadTelegramBook(id: string, libraryId?: string): Promise<void> {
  await apiFetch(`/telegram-monitor/books/${id}/download`, {
    method: "POST",
    body: JSON.stringify({ library_id: libraryId ?? null }),
  });
}

export async function dismissTelegramBook(id: string): Promise<void> {
  await apiFetch(`/telegram-monitor/books/${id}`, { method: "DELETE" });
}

export async function fetchTelegramAvailable(): Promise<TelegramAvailableGroupDto[]> {
  return apiFetch<TelegramAvailableGroupDto[]>("/telegram-monitor/available");
}

export async function fetchTelegramDownloads(): Promise<TelegramDownloadItemDto[]> {
  return apiFetch<TelegramDownloadItemDto[]>("/telegram-monitor/downloads");
}

export type TelegramSearchResultDto = {
  id: string;
  channel_username: string;
  filename: string;
  file_size: number | null;
  volume_number: number | null;
  series_name: string | null;
  status: string;
  created_at: string;
};

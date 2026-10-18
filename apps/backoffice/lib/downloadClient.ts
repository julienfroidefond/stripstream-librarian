/**
 * Pure helpers describing the download-client choice, shared by the UI and the
 * Settings server page.
 *
 * They live outside the `"use client"` component module so Server Components can
 * call `readDownloadClientSetting` / `isClientConfigured` directly (importing a
 * plain function from a client module would hand them a client reference).
 */

import type { DownloadClient } from "./api";

/** Brand names of the supported download clients — never translated. */
export const DOWNLOAD_CLIENT_LABELS: Record<DownloadClient, string> = {
  qbittorrent: "qBittorrent",
  alldebrid: "AllDebrid",
};

/** Supported download clients, in the order used to pick a default fallback. */
export const DOWNLOAD_CLIENTS: DownloadClient[] = ["qbittorrent", "alldebrid"];

/** `app_settings` key holding the admin's chosen download client. */
export const DOWNLOAD_CLIENT_SETTING_KEY = "download_client";

/** Stored shape of the `download_client` setting. */
export interface DownloadClientSetting {
  client?: DownloadClient;
}

/** Extract the stored client choice, ignoring unknown or malformed values. */
export function readDownloadClientSetting(data: unknown): DownloadClient | null {
  const client = (data as DownloadClientSetting | null)?.client;
  return DOWNLOAD_CLIENTS.includes(client as DownloadClient) ? (client as DownloadClient) : null;
}

/** Backoffice proxy endpoint accepting a snatch for the given client. */
export function downloadClientAddEndpoint(client: DownloadClient): string {
  return `/api/${client}/add`;
}

/**
 * Decide whether a client is usable from its settings payload.
 *
 * qBittorrent needs a URL and a username; AllDebrid must be enabled and hold a
 * non-empty API key. The API never returns the real AllDebrid key (it is
 * redacted as a non-empty placeholder), so a non-empty string is enough here.
 */
export function isClientConfigured(client: DownloadClient, data: Record<string, unknown> | null): boolean {
  if (!data) return false;
  if (client === "qbittorrent") {
    return !!(typeof data.url === "string" && data.url.trim() && typeof data.username === "string" && data.username.trim());
  }
  return data.enabled === true && typeof data.api_key === "string" && data.api_key.trim().length > 0;
}

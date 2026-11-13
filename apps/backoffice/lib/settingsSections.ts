/**
 * Defaults and validation rules for every `app_settings` section edited by the
 * Backoffice settings page.
 *
 * The API stores whole JSON sections and only merges defaults for a few runtime
 * keys, so the browser seeds the same defaults here. Two consequences:
 *
 * - a first save always sends a complete section (Prowlarr needs `url` and
 *   `api_key` fields to deserialize, even when disabled);
 * - the draft baseline is the seeded value, so saving an untouched form is a
 *   no-op instead of rewriting the row.
 *
 * Validation owns *intent* — enabling an integration without its required
 * fields is rejected with an explicit message instead of being silently
 * dropped before the request.
 */

import type { DownloadClient } from "./api";
import { DOWNLOAD_CLIENTS, type DownloadClientSetting } from "./downloadClient";

export interface LimitsSettings {
  concurrent_renders: number;
  timeout_seconds: number;
  analysis_timeout_seconds: number;
  rate_limit_per_second: number;
  concurrent_telegram_downloads: number;
  telegram_download_connections: number;
  concurrent_ddl_provider_downloads: number;
  ddl_download_connections: number;
  download_max_attempts: number;
  download_stall_timeout_seconds: number;
  download_timeout_minutes: number;
  concurrent_alldebrid_downloads: number;
  alldebrid_stall_timeout_minutes: number;
}

export interface ImageProcessingSettings {
  format: string;
  quality: number;
  filter: string;
  max_width: number;
}

export interface CacheSettings {
  enabled: boolean;
  directory: string;
  max_size_mb: number;
  memory_max_size_mb: number;
}

export interface ThumbnailSettings {
  enabled: boolean;
  width: number;
  height: number;
  quality: number;
  format: string;
  directory: string;
}

export interface DownloadsEnabledSettings {
  enabled: boolean;
}

export interface ProwlarrSettings {
  url: string;
  api_key: string;
  categories: number[];
  rss_poll_interval_minutes: number;
  enabled: boolean;
}

export interface QbittorrentSettings {
  url: string;
  username: string;
  password: string;
  enabled: boolean;
}

export interface AlldebridSettings {
  enabled: boolean;
  /** Write-only: the API returns `********` once a key is stored. */
  api_key?: string;
}

export const TELEGRAM_EVENT_KEYS = [
  "scan_completed",
  "scan_failed",
  "scan_cancelled",
  "thumbnail_completed",
  "thumbnail_failed",
  "conversion_completed",
  "conversion_failed",
  "metadata_approved",
  "metadata_batch_completed",
  "metadata_batch_failed",
  "metadata_refresh_completed",
  "metadata_refresh_failed",
  "reading_status_match_completed",
  "reading_status_match_failed",
  "reading_status_push_completed",
  "reading_status_push_failed",
  "download_detection_completed",
  "download_detection_partial",
  "download_detection_failed",
] as const;

export type TelegramEventKey = (typeof TELEGRAM_EVENT_KEYS)[number];
export type TelegramEvents = Record<TelegramEventKey, boolean>;

export interface TelegramSettings {
  bot_token: string;
  chat_id: string;
  enabled: boolean;
  events: TelegramEvents;
}

export interface MetadataProvidersSettings {
  default_provider: string;
  metadata_language: string;
  google_books?: { api_key?: string };
  comicvine?: { api_key?: string };
}

export interface AiTaggingSettings {
  enabled: boolean;
  base_url: string;
  api_key: string;
  model: string;
  max_tags: number;
  prompt: string;
  reading_lists_enabled: boolean;
  reading_list_prompt: string;
}

export interface AnilistSettings {
  client_id: string;
  access_token: string;
  user_id: number | null;
  local_user_id: string;
}

export interface KomgaSettings {
  url: string;
  username: string;
  user_id: string;
}

export interface RenameFormatSettings {
  rename_format: string;
  rename_format_hs: string;
  rename_format_int: string;
  rename_format_oneshot: string;
}

export const DEFAULT_TELEGRAM_EVENTS: TelegramEvents = {
  scan_completed: true,
  scan_failed: true,
  scan_cancelled: true,
  thumbnail_completed: true,
  thumbnail_failed: true,
  conversion_completed: true,
  conversion_failed: true,
  metadata_approved: true,
  metadata_batch_completed: true,
  metadata_batch_failed: true,
  metadata_refresh_completed: true,
  metadata_refresh_failed: true,
  reading_status_match_completed: true,
  reading_status_match_failed: true,
  reading_status_push_completed: true,
  reading_status_push_failed: true,
  download_detection_completed: true,
  download_detection_partial: true,
  download_detection_failed: true,
};

export const DEFAULT_AI_PROMPT = "Suggest up to {{max_tags}} relevant genres for each comic or manga series. Usually return only 1 to 3 genres; do not fill the limit when fewer genres fit.\n\nYou MUST choose tags only from this existing genre list, preserving the exact spelling and casing: {{genres}}. Never create or paraphrase a genre.\n\nUse the supplied metadata to disambiguate the series. For a recognizable, well-known title, you may use reliable general knowledge even when its metadata is sparse. Do not make uncertain associations: omit a series if you cannot identify it with confidence. Do not select a format, medium, age category, or generic label merely because the series is a comic (for example 'BD' or 'Books/Comics'). Select those labels only if they are genuinely relevant genres for the series. Return only JSON in the form {\"suggestions\":[{\"series_id\":\"uuid\",\"tags\":[\"existing genre\"]}]}.\n\nSeries: {{series}}";

export const DEFAULT_AI_LISTS_PROMPT = "You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).\n\nAnalyse the existing lists: {{lists}}\n\nAnalyse these series, which are not part of any list yet: {{series}}\n\nReturn at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.\n\nAlso propose additions of available series to existing lists when they clearly belong there. Never propose a series that is already in a list.\n\nEvery new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.\n\nReturn only JSON in the form {\"additions\":[{\"list_id\":\"uuid\",\"reason\":\"...\",\"series_ids\":[\"uuid\"]}],\"new_lists\":[{\"name\":\"...\",\"description\":\"...\",\"reason\":\"...\",\"series_ids\":[\"uuid\",\"uuid\"]}]}. Use only the series ids supplied above.";

export const DEFAULT_RENAME_FORMATS: RenameFormatSettings = {
  rename_format: "{series_name} - T{volume_padded} - {title}",
  rename_format_hs: "{series_name} - HS {volume_padded}",
  rename_format_int: "{series_name} - INT {volume_padded}",
  rename_format_oneshot: "{series_name}",
};

/** Defaults for every section the settings page can write. */
export const SETTINGS_SECTION_DEFAULTS: Record<string, unknown> = {
  downloads_enabled: { enabled: false } satisfies DownloadsEnabledSettings,
  prowlarr: {
    url: "",
    api_key: "",
    categories: [7030, 7020],
    rss_poll_interval_minutes: 30,
    enabled: true,
  } satisfies ProwlarrSettings,
  qbittorrent: {
    url: "",
    username: "",
    password: "",
    enabled: true,
  } satisfies QbittorrentSettings,
  alldebrid: { enabled: false } satisfies AlldebridSettings,
  telegram: {
    bot_token: "",
    chat_id: "",
    enabled: false,
    events: DEFAULT_TELEGRAM_EVENTS,
  } satisfies TelegramSettings,
  ai_tagging: {
    enabled: false,
    base_url: "https://openrouter.ai/api/v1",
    api_key: "",
    model: "openrouter/free",
    max_tags: 5,
    prompt: DEFAULT_AI_PROMPT,
    reading_lists_enabled: false,
    reading_list_prompt: DEFAULT_AI_LISTS_PROMPT,
  } satisfies AiTaggingSettings,
  metadata_providers: {
    default_provider: "google_books",
    metadata_language: "en",
  } satisfies MetadataProvidersSettings,
  anilist: {
    client_id: "",
    access_token: "",
    user_id: null,
    local_user_id: "",
  } satisfies AnilistSettings,
  komga: { url: "", username: "", user_id: "" } satisfies KomgaSettings,
  download_client: {} satisfies DownloadClientSetting,
  limits: {
    concurrent_renders: 4,
    timeout_seconds: 12,
    analysis_timeout_seconds: 120,
    rate_limit_per_second: 120,
    concurrent_telegram_downloads: 2,
    telegram_download_connections: 4,
    concurrent_ddl_provider_downloads: 2,
    ddl_download_connections: 4,
    download_max_attempts: 5,
    download_stall_timeout_seconds: 180,
    download_timeout_minutes: 30,
    concurrent_alldebrid_downloads: 4,
    alldebrid_stall_timeout_minutes: 30,
  } satisfies LimitsSettings,
  cache: {
    enabled: true,
    directory: "/tmp/stripstream-image-cache",
    max_size_mb: 10000,
    memory_max_size_mb: 128,
  } satisfies CacheSettings,
  thumbnail: {
    enabled: true,
    width: 300,
    height: 400,
    quality: 80,
    format: "webp",
    directory: "/data/thumbnails",
  } satisfies ThumbnailSettings,
  image_processing: {
    format: "webp",
    quality: 85,
    filter: "lanczos3",
    max_width: 2160,
  } satisfies ImageProcessingSettings,
  ...DEFAULT_RENAME_FORMATS,
};

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null) return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

/** Recursively merge stored values over the defaults; arrays replace. */
function mergeDefaults(defaultValue: unknown, stored: unknown): unknown {
  // A nullish stored field means "unset", not "override the default with null":
  // a whole nested object (Telegram `events`, limits) must keep its defaults.
  if (stored === null || stored === undefined) return structuredClone(defaultValue);
  if (!isPlainObject(defaultValue) || !isPlainObject(stored)) return stored;
  const merged: Record<string, unknown> = { ...defaultValue };
  for (const [key, value] of Object.entries(stored)) {
    merged[key] = key in defaultValue ? mergeDefaults(defaultValue[key], value) : value;
  }
  return merged;
}

/**
 * Seed every known section with its defaults, then layer the stored values on
 * top. Stored `null`/`undefined` sections fall back to the default.
 */
export function seedSettingsSections(stored: Record<string, unknown>): Record<string, unknown> {
  const seeded: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(SETTINGS_SECTION_DEFAULTS)) {
    seeded[key] = structuredClone(value);
  }
  for (const [key, value] of Object.entries(stored)) {
    if (value === null || value === undefined) continue;
    seeded[key] = key in seeded ? mergeDefaults(seeded[key], value) : value;
  }
  return seeded;
}

function stringField(value: unknown, field: string): string {
  if (!isPlainObject(value)) return "";
  const fieldValue = value[field];
  return typeof fieldValue === "string" ? fieldValue.trim() : "";
}

function enabled(value: unknown): boolean {
  return isPlainObject(value) && value.enabled === true;
}

function isDownloadClient(value: unknown): value is DownloadClient {
  return typeof value === "string" && DOWNLOAD_CLIENTS.includes(value as DownloadClient);
}

/**
 * Only a *transition* to enabled needs the required fields. An integration
 * that is already enabled must stay editable field by field: blocking every
 * partial save would make the first configuration impossible (typing the URL,
 * tabbing to the API key, and so on).
 */
function missingWhileEnabling(
  value: unknown,
  acknowledged: unknown,
  fields: readonly string[],
): boolean {
  return enabled(value) && !enabled(acknowledged) && fields.some((field) => !stringField(value, field));
}

/** Translation key of the validation error, or null when the section is valid. */
export type SettingsValidationMessage =
  | "settings.validationProwlarr"
  | "settings.validationQbittorrent"
  | "settings.validationAlldebrid"
  | "settings.validationTelegram"
  | "settings.validationAi"
  | "settings.validationCacheDirectory"
  | "settings.validationThumbnailDirectory"
  | "settings.validationDownloadClient";

/**
 * Reject a save that would enable an integration without the fields it needs.
 * These rules mirror the cross-field guards the cards used to apply before
 * saving; they only block enabled states, so a section can always be disabled.
 */
export function validateSettingsSection(
  key: string,
  value: unknown,
  acknowledged: unknown = undefined,
): SettingsValidationMessage | null {
  switch (key) {
    case "prowlarr":
      if (missingWhileEnabling(value, acknowledged, ["url", "api_key"])) {
        return "settings.validationProwlarr";
      }
      return null;
    case "qbittorrent":
      if (missingWhileEnabling(value, acknowledged, ["url", "username"])) {
        return "settings.validationQbittorrent";
      }
      return null;
    case "alldebrid":
      if (missingWhileEnabling(value, acknowledged, ["api_key"])) {
        return "settings.validationAlldebrid";
      }
      return null;
    case "telegram":
      if (missingWhileEnabling(value, acknowledged, ["bot_token", "chat_id"])) {
        return "settings.validationTelegram";
      }
      return null;
    case "ai_tagging":
      if (missingWhileEnabling(value, acknowledged, ["base_url", "model"])) {
        return "settings.validationAi";
      }
      return null;
    case "cache":
      if (!stringField(value, "directory")) return "settings.validationCacheDirectory";
      return null;
    case "thumbnail":
      if (!stringField(value, "directory")) return "settings.validationThumbnailDirectory";
      return null;
    case "download_client": {
      const client = isPlainObject(value) ? value.client : undefined;
      if (client !== undefined && client !== null && !isDownloadClient(client)) {
        return "settings.validationDownloadClient";
      }
      return null;
    }
    default:
      return null;
  }
}

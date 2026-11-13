/**
 * Library settings form: what the modal edits, which writes a submission needs
 * and how they are applied. Pure (no React); the runner takes its writer as a
 * parameter so it can be tested without the network.
 */

import type { LibraryDto } from "../api/libraries";

/** The editable slice of a library, normalised (empty provider ⇒ `null`). */
export type LibrarySettings = {
  monitor_enabled: boolean;
  scan_mode: string;
  watcher_enabled: boolean;
  metadata_refresh_mode: string;
  download_detection_mode: string;
  metadata_provider: string | null;
  fallback_metadata_provider: string | null;
  reading_status_provider: string | null;
  reading_status_push_mode: string;
  tags: string[];
};

/** One PATCH endpoint of `/api/libraries/[id]/…`. */
export type LibraryWrite =
  | {
      kind: "monitoring";
      monitor_enabled: boolean;
      scan_mode: string;
      watcher_enabled: boolean;
      metadata_refresh_mode: string;
      download_detection_mode: string;
    }
  | { kind: "metadataProvider"; provider: string | null; fallbackProvider: string | null }
  | { kind: "readingStatusProvider"; provider: string | null; pushMode: string }
  | { kind: "tags"; tags: string[] };

export type LibraryWriteKind = LibraryWrite["kind"];

export const PREDEFINED_LIBRARY_TAGS = [
  "manga",
  "manhwa",
  "manhua",
  "bd-jeunesse",
  "bd-adulte",
  "bd-franco-belge",
  "comics",
  "webtoon",
  "light-novel",
] as const;

const DEFAULT_MODE = "manual";

function optionalString(value: unknown): string | null {
  return typeof value === "string" && value !== "" ? value : null;
}

export function librarySettingsOf(library: LibraryDto): LibrarySettings {
  return {
    monitor_enabled: library.monitor_enabled,
    scan_mode: library.scan_mode,
    watcher_enabled: library.watcher_enabled,
    metadata_refresh_mode: library.metadata_refresh_mode,
    download_detection_mode: library.download_detection_mode ?? DEFAULT_MODE,
    metadata_provider: optionalString(library.metadata_provider),
    fallback_metadata_provider: optionalString(library.fallback_metadata_provider),
    reading_status_provider: optionalString(library.reading_status_provider),
    reading_status_push_mode: library.reading_status_push_mode || DEFAULT_MODE,
    tags: library.tags ?? [],
  };
}

/** Reads the modal's form fields; tags are held in component state. */
export function librarySettingsFromForm(formData: FormData, tags: string[]): LibrarySettings {
  const text = (name: string) => formData.get(name);
  return {
    monitor_enabled: text("monitor_enabled") === "true",
    watcher_enabled: text("watcher_enabled") === "true",
    scan_mode: optionalString(text("scan_mode")) ?? DEFAULT_MODE,
    metadata_refresh_mode: optionalString(text("metadata_refresh_mode")) ?? DEFAULT_MODE,
    download_detection_mode: optionalString(text("download_detection_mode")) ?? DEFAULT_MODE,
    metadata_provider: optionalString(text("metadata_provider")),
    fallback_metadata_provider: optionalString(text("fallback_metadata_provider")),
    reading_status_provider: optionalString(text("reading_status_provider")),
    reading_status_push_mode: optionalString(text("reading_status_push_mode")) ?? DEFAULT_MODE,
    tags,
  };
}

function sameTags(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const left = [...a].sort();
  const right = [...b].sort();
  return left.every((tag, i) => tag === right[i]);
}

/**
 * The writes needed to go from `before` to `form`, in a fixed order. An
 * untouched section produces no write, so an unchanged form produces none.
 */
export function diffLibrarySettings(before: LibrarySettings, form: LibrarySettings): LibraryWrite[] {
  const writes: LibraryWrite[] = [];

  if (
    before.monitor_enabled !== form.monitor_enabled ||
    before.scan_mode !== form.scan_mode ||
    before.watcher_enabled !== form.watcher_enabled ||
    before.metadata_refresh_mode !== form.metadata_refresh_mode ||
    before.download_detection_mode !== form.download_detection_mode
  ) {
    writes.push({
      kind: "monitoring",
      monitor_enabled: form.monitor_enabled,
      scan_mode: form.scan_mode,
      watcher_enabled: form.watcher_enabled,
      metadata_refresh_mode: form.metadata_refresh_mode,
      download_detection_mode: form.download_detection_mode,
    });
  }

  if (
    before.metadata_provider !== form.metadata_provider ||
    before.fallback_metadata_provider !== form.fallback_metadata_provider
  ) {
    writes.push({
      kind: "metadataProvider",
      provider: form.metadata_provider,
      fallbackProvider: form.fallback_metadata_provider,
    });
  }

  if (
    before.reading_status_provider !== form.reading_status_provider ||
    before.reading_status_push_mode !== form.reading_status_push_mode
  ) {
    writes.push({
      kind: "readingStatusProvider",
      provider: form.reading_status_provider,
      pushMode: form.reading_status_push_mode,
    });
  }

  if (!sameTags(before.tags, form.tags)) {
    writes.push({ kind: "tags", tags: form.tags });
  }

  return writes;
}

export type LibraryWriter = (libraryId: string, write: LibraryWrite) => Promise<unknown>;

export type LibraryWritesResult =
  | { status: "ok"; applied: LibraryWriteKind[] }
  | { status: "failed"; applied: LibraryWriteKind[]; failed: LibraryWriteKind; error: unknown };

/**
 * Applies the writes one after the other and stops at the first failure: the
 * writes already applied stay applied (the API has no transaction across these
 * endpoints), the remaining ones are not sent, and the caller gets the failing
 * section and the error.
 */
export async function applyLibraryWrites(
  libraryId: string,
  writes: LibraryWrite[],
  writer: LibraryWriter,
): Promise<LibraryWritesResult> {
  const applied: LibraryWriteKind[] = [];
  for (const write of writes) {
    try {
      await writer(libraryId, write);
    } catch (error) {
      return { status: "failed", applied, failed: write.kind, error };
    }
    applied.push(write.kind);
  }
  return { status: "ok", applied };
}

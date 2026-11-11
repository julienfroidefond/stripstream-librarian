/** Shared status badge labels/classes for every download renderer. */

import type { TranslationKey } from "@/lib/i18n/fr";

export type DownloadTranslate = (
  key: TranslationKey,
  vars?: Record<string, string | number>,
) => string;

export function downloadStatusLabel(status: string, t: DownloadTranslate): string {
  const map: Record<string, TranslationKey> = {
    queued: "downloads.status.queued",
    downloading: "downloads.status.downloading",
    completed: "downloads.status.completed",
    importing: "downloads.status.importing",
    imported: "downloads.status.imported",
    partial: "downloads.status.partial",
    no_files_imported: "downloads.status.noFilesImported",
    error: "downloads.status.error",
  };
  return t(map[status] ?? status);
}

export function downloadStatusClass(status: string): string {
  switch (status) {
    case "queued":
      return "bg-muted/50 text-muted-foreground";
    case "downloading":
      return "bg-primary/10 text-primary";
    case "completed":
      return "bg-warning/10 text-warning";
    case "importing":
      return "bg-primary/10 text-primary";
    case "imported":
      return "bg-success/10 text-success";
    case "partial":
      return "bg-warning/10 text-warning";
    case "no_files_imported":
      return "bg-destructive/10 text-destructive";
    case "error":
      return "bg-destructive/10 text-destructive";
    default:
      return "bg-muted/30 text-muted-foreground";
  }
}

import type { IconName } from "@/app/components/ui";
import type { TranslationKey } from "@/lib/i18n/fr";

import {
  triggerDdlSearch,
  triggerDownloadDetection,
  triggerFullRebuild,
  triggerMetadataBatch,
  triggerMetadataRefresh,
  triggerMetadataRefreshAll,
  triggerMetadataRematch,
  triggerRatingPull,
  triggerReadingStatusMatch,
  triggerReadingStatusPush,
  triggerRebuild,
  triggerRescan,
  triggerRssPoll,
  triggerTelegramSync,
  triggerTelegramSyncIncremental,
  triggerThumbnailsRebuild,
  triggerThumbnailsRegenerate,
} from "../actions";

export type JobActionGroupId =
  | "indexation"
  | "thumbnails"
  | "metadata"
  | "reading"
  | "prowlarr"
  | "ddl"
  | "telegram";

export type JobActionTone = "primary" | "warning" | "destructive";

export interface JobActionGroup {
  id: JobActionGroupId;
  labelKey: TranslationKey;
  icon: IconName;
}

export interface JobAction {
  id: string;
  group: JobActionGroupId;
  labelKey: TranslationKey;
  descriptionKey: TranslationKey;
  icon: IconName;
  tone: JobActionTone;
  /** The action reads `library_id` from the form. */
  usesLibrary: boolean;
  /** The action only works on selected libraries (not "all libraries"). */
  requiresLibrary?: boolean;
  /** Ask for an explicit confirmation before launching. */
  confirm?: boolean;
  /** Promoted as a one-click button in the compact toolbar. */
  quick?: boolean;
  /** Shorter label used on the quick button (the tooltip keeps the full label). */
  quickLabelKey?: TranslationKey;
  /** Precise tooltip shown on the quick button (what it does, not just its name). */
  quickHintKey?: TranslationKey;
  action: (formData: FormData) => Promise<void>;
}

export interface JobActionFlags {
  readingStatus: boolean;
  prowlarr: boolean;
  ddl: boolean;
  telegram: boolean;
}

export const JOB_ACTION_GROUPS: JobActionGroup[] = [
  { id: "indexation", labelKey: "jobs.groupIndexation", icon: "folder" },
  { id: "thumbnails", labelKey: "jobs.groupThumbnails", icon: "image" },
  { id: "metadata", labelKey: "jobs.groupMetadata", icon: "tag" },
  { id: "reading", labelKey: "jobs.groupReadingStatus", icon: "check" },
  { id: "prowlarr", labelKey: "jobs.groupProwlarr", icon: "download" },
  { id: "ddl", labelKey: "jobs.groupDdl", icon: "globe" },
  { id: "telegram", labelKey: "jobs.groupTelegram", icon: "send" },
];

export function getJobActions(flags: JobActionFlags): JobAction[] {
  const actions: JobAction[] = [
    {
      id: "rebuild",
      group: "indexation",
      labelKey: "jobs.rebuild",
      descriptionKey: "jobs.rebuildShort",
      icon: "refresh",
      tone: "primary",
      usesLibrary: true,
      quick: true,
      quickHintKey: "jobs.rebuildQuickHint",
      action: triggerRebuild,
    },
    {
      id: "rescan",
      group: "indexation",
      labelKey: "jobs.rescan",
      descriptionKey: "jobs.rescanShort",
      icon: "search",
      tone: "primary",
      usesLibrary: true,
      action: triggerRescan,
    },
    {
      id: "fullRebuild",
      group: "indexation",
      labelKey: "jobs.fullRebuild",
      descriptionKey: "jobs.fullRebuildShort",
      icon: "warning",
      tone: "destructive",
      usesLibrary: true,
      confirm: true,
      action: triggerFullRebuild,
    },
    {
      id: "thumbnailsRebuild",
      group: "thumbnails",
      labelKey: "jobs.generateThumbnails",
      descriptionKey: "jobs.generateThumbnailsShort",
      icon: "image",
      tone: "primary",
      usesLibrary: true,
      quick: true,
      quickLabelKey: "jobs.generateThumbnailsQuick",
      quickHintKey: "jobs.generateThumbnailsQuickHint",
      action: triggerThumbnailsRebuild,
    },
    {
      id: "thumbnailsRegenerate",
      group: "thumbnails",
      labelKey: "jobs.regenerateThumbnails",
      descriptionKey: "jobs.regenerateThumbnailsShort",
      icon: "image",
      tone: "warning",
      usesLibrary: true,
      confirm: true,
      action: triggerThumbnailsRegenerate,
    },
    {
      id: "metadataBatch",
      group: "metadata",
      labelKey: "jobs.batchMetadata",
      descriptionKey: "jobs.batchMetadataShort",
      icon: "tag",
      tone: "primary",
      usesLibrary: true,
      action: triggerMetadataBatch,
    },
    {
      id: "metadataRematch",
      group: "metadata",
      labelKey: "jobs.rematchMetadata",
      descriptionKey: "jobs.rematchMetadataShort",
      icon: "refresh",
      tone: "warning",
      usesLibrary: true,
      requiresLibrary: true,
      confirm: true,
      action: triggerMetadataRematch,
    },
    {
      id: "metadataRefresh",
      group: "metadata",
      labelKey: "jobs.refreshMetadata",
      descriptionKey: "jobs.refreshMetadataShort",
      icon: "tag",
      tone: "primary",
      usesLibrary: true,
      quick: true,
      quickLabelKey: "jobs.refreshMetadataQuick",
      quickHintKey: "jobs.refreshMetadataQuickHint",
      action: triggerMetadataRefresh,
    },
    {
      id: "metadataRefreshAll",
      group: "metadata",
      labelKey: "jobs.refreshMetadataAll",
      descriptionKey: "jobs.refreshMetadataAllShort",
      icon: "refresh",
      tone: "warning",
      usesLibrary: true,
      confirm: true,
      action: triggerMetadataRefreshAll,
    },
    {
      id: "readingStatusMatch",
      group: "reading",
      labelKey: "jobs.matchReadingStatus",
      descriptionKey: "jobs.matchReadingStatusShort",
      icon: "link",
      tone: "primary",
      usesLibrary: true,
      action: triggerReadingStatusMatch,
    },
    {
      id: "readingStatusPush",
      group: "reading",
      labelKey: "jobs.pushReadingStatus",
      descriptionKey: "jobs.pushReadingStatusShort",
      icon: "send",
      tone: "primary",
      usesLibrary: true,
      action: triggerReadingStatusPush,
    },
    {
      id: "ratingPull",
      group: "reading",
      labelKey: "jobs.pullRating",
      descriptionKey: "jobs.pullRatingShort",
      icon: "star",
      tone: "primary",
      usesLibrary: false,
      action: triggerRatingPull,
    },
    {
      id: "downloadDetection",
      group: "prowlarr",
      labelKey: "jobs.downloadDetection",
      descriptionKey: "jobs.downloadDetectionShort",
      icon: "download",
      tone: "primary",
      usesLibrary: true,
      quick: true,
      quickLabelKey: "jobs.downloadDetectionQuick",
      quickHintKey: "jobs.downloadDetectionQuickHint",
      action: triggerDownloadDetection,
    },
    {
      id: "rssPoll",
      group: "prowlarr",
      labelKey: "jobs.rssPoll",
      descriptionKey: "jobs.rssPollShort",
      icon: "globe",
      tone: "primary",
      usesLibrary: true,
      action: triggerRssPoll,
    },
    {
      id: "ddlSearch",
      group: "ddl",
      labelKey: "jobs.ddlSearch",
      descriptionKey: "jobs.ddlSearchShort",
      icon: "search",
      tone: "primary",
      usesLibrary: false,
      quick: true,
      quickLabelKey: "jobs.ddlSearchQuick",
      quickHintKey: "jobs.ddlSearchQuickHint",
      action: triggerDdlSearch,
    },
    {
      id: "telegramSync",
      group: "telegram",
      labelKey: "jobs.telegramSync",
      descriptionKey: "jobs.telegramSyncShort",
      icon: "send",
      tone: "primary",
      usesLibrary: false,
      quick: true,
      quickLabelKey: "jobs.telegramSyncQuick",
      quickHintKey: "jobs.telegramSyncQuickHint",
      action: triggerTelegramSync,
    },
    {
      id: "telegramSyncIncremental",
      group: "telegram",
      labelKey: "jobs.telegramSyncIncremental",
      descriptionKey: "jobs.telegramSyncIncrementalShort",
      icon: "clock",
      tone: "primary",
      usesLibrary: false,
      action: triggerTelegramSyncIncremental,
    },
  ];

  return actions.filter((action) => {
    if (action.group === "reading") return flags.readingStatus;
    if (action.group === "prowlarr") return flags.prowlarr;
    if (action.group === "ddl") return flags.ddl;
    if (action.group === "telegram") return flags.telegram;
    return true;
  });
}

export const TONE_BADGE: Record<JobActionTone, string> = {
  primary: "bg-primary/10 text-primary",
  warning: "bg-warning/15 text-warning",
  destructive: "bg-destructive/10 text-destructive",
};

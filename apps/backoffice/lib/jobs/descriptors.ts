/**
 * Job-type descriptors: the single registry of what the backoffice knows about
 * each background job type.
 *
 * The jobs list and the job detail page both read their labels, category,
 * replayability, progress copy and card plan from here. The server catalog
 * (`GET /index/job-types`) stays authoritative for `category` and `replayable`
 * when it is available; the registry supplies the presentation and the
 * report/card wiring.
 *
 * The module is pure TypeScript (no React, no JSX) and imports no API runtime:
 * `JobsList` is a Client Component, so it can only consume erased type imports.
 * Report fetchers are injected through `loaders` (see
 * `lib/jobs/serverReports.ts`), which keeps the loaders centralized on the
 * server side while the descriptors stay importable everywhere.
 */

import type {
  DownloadDetectionReportDto,
  DownloadDetectionResultDto,
  JobTypeCatalogDto,
  MetadataBatchReportDto,
  MetadataBatchResultDto,
  MetadataRefreshReportDto,
  ReadingStatusMatchReportDto,
  ReadingStatusMatchResultDto,
  ReadingStatusPushReportDto,
  ReadingStatusPushResultDto,
} from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import type { TranslationKey } from "@/lib/i18n/fr";

/** Functional family shared with the Rust job registry (`crates/jobs`). */
export const JOB_CATEGORIES = [
  "scan",
  "thumbnail",
  "conversion",
  "metadata",
  "reading",
  "download",
  "search",
  "other",
] as const;

export type JobCategory = (typeof JOB_CATEGORIES)[number];

/** Result/report card families backed by a report endpoint. */
export type JobReportKind =
  | "metadata_batch"
  | "metadata_refresh"
  | "reading_status_match"
  | "reading_status_push"
  | "download_detection"
  | "rss_poll";

/** Card families rendered from `index_jobs.stats_json` only. */
export type JobStatCardKind = "telegram_sync" | "telegram_sync_incremental";

export type JobResultCardKind = JobReportKind | JobStatCardKind;

const JOB_REPORT_KINDS: readonly JobReportKind[] = [
  "metadata_batch",
  "metadata_refresh",
  "reading_status_match",
  "reading_status_push",
  "download_detection",
  "rss_poll",
];

export function isJobReportKind(kind: JobResultCardKind): kind is JobReportKind {
  return (JOB_REPORT_KINDS as readonly string[]).includes(kind);
}

/** Data loaded for one job, discriminated by the card family that consumes it. */
export type JobReport =
  | { kind: "metadata_batch"; report: MetadataBatchReportDto | null; results: MetadataBatchResultDto[] }
  | { kind: "metadata_refresh"; report: MetadataRefreshReportDto | null }
  | {
      kind: "reading_status_match";
      report: ReadingStatusMatchReportDto | null;
      results: ReadingStatusMatchResultDto[];
    }
  | {
      kind: "reading_status_push";
      report: ReadingStatusPushReportDto | null;
      results: ReadingStatusPushResultDto[];
    }
  | {
      kind: "download_detection";
      report: DownloadDetectionReportDto | null;
      results: DownloadDetectionResultDto[];
      errors: DownloadDetectionResultDto[];
    }
  | {
      kind: "rss_poll";
      report: DownloadDetectionReportDto | null;
      results: DownloadDetectionResultDto[];
      errors: DownloadDetectionResultDto[];
    };

/** One report fetch (or fetch group) per report-backed card family. */
export type JobReportLoaders = Record<JobReportKind, (jobId: string) => Promise<JobReport>>;

/** Loads the report data a descriptor's result card needs. */
export interface JobReportLoader {
  load: (jobId: string) => Promise<JobReport>;
}

/**
 * Progress card copy for a job type:
 * - `status`: title/description follow the running phase (phase 1 / 2a / 2b),
 *   with an optional fixed title override;
 * - `title`: fixed title, no description (thumbnail-only jobs);
 * - `fixed`: fixed title and description (report jobs).
 */
export type JobProgressCopy =
  | { mode: "status"; title?: string }
  | { mode: "title"; title: string }
  | { mode: "fixed"; title: string; description: string };

/** What the detail page renders for a job type, beyond the shared cards. */
export interface JobResultCardPlan {
  /** Report/results card family; `null` when the type adds no result card. */
  kind: JobResultCardKind | null;
  /** Generic progress card (hidden for full Telegram syncs: the results are the output). */
  showProgress: boolean;
  /** Index statistics card (scanned/indexed/removed counters). */
  showIndexStats: boolean;
  /** Thumbnail-only presentation (thumbnail stats card, "generated" counters). */
  thumbnailOnly: boolean;
  /** Copy of the progress card. */
  progress: JobProgressCopy;
}

export interface JobTypeDescriptor {
  /** Database value stored in `index_jobs.type`. */
  type: string;
  /** Long label used on the detail page and in overview cards. */
  label: string;
  /** One-line explanation of what the job does. */
  description: string;
  /** Functional family (catalog value when available). */
  category: JobCategory;
  /** Whether the admin UI can create a fresh job of this type. */
  replayable: boolean;
  /** Report loader, `null` when the type has no report-backed card. */
  report: JobReportLoader | null;
  /** Detail-page card plan. */
  resultCard: JobResultCardPlan;
}

type ProgressSpec =
  | { mode: "status"; titleKey?: TranslationKey }
  | { mode: "title"; titleKey: TranslationKey }
  | { mode: "fixed"; titleKey: TranslationKey; descriptionKey: TranslationKey };

interface CardPlanSpec {
  kind: JobResultCardKind | null;
  showProgress?: boolean;
  showIndexStats?: boolean;
  thumbnailOnly?: boolean;
  progress: ProgressSpec;
}

interface JobTypeSpec {
  type: string;
  labelKey: TranslationKey;
  descriptionKey: TranslationKey;
  category: JobCategory;
  replayable: boolean;
  cards: CardPlanSpec;
}

const STATUS_PROGRESS: ProgressSpec = { mode: "status" };

/** Plain indexing/conversion jobs: status-driven progress, index stats. */
const INDEX_CARDS: CardPlanSpec = { kind: null, progress: STATUS_PROGRESS };

/** Report jobs: fixed progress copy, no index statistics. */
const reportCards = (
  kind: JobReportKind,
  titleKey: TranslationKey,
  descriptionKey: TranslationKey
): CardPlanSpec => ({
  kind,
  showIndexStats: false,
  progress: { mode: "fixed", titleKey, descriptionKey },
});

/** Thumbnail-only jobs: fixed title, no progress description, thumbnail stats. */
const THUMBNAIL_CARDS: CardPlanSpec = {
  kind: null,
  showIndexStats: false,
  thumbnailOnly: true,
  progress: { mode: "title", titleKey: "jobType.thumbnail_rebuild" },
};

/**
 * Every job type the backoffice renders. Keep the label/description keys in
 * sync with the detail page copy; the server catalog covers categories and
 * replayability at runtime.
 */
const JOB_TYPE_SPECS: readonly JobTypeSpec[] = [
  {
    type: "rebuild",
    labelKey: "jobType.rebuildLabel",
    descriptionKey: "jobType.rebuildDesc",
    category: "scan",
    replayable: true,
    cards: INDEX_CARDS,
  },
  {
    type: "full_rebuild",
    labelKey: "jobType.full_rebuildLabel",
    descriptionKey: "jobType.full_rebuildDesc",
    category: "scan",
    replayable: true,
    cards: INDEX_CARDS,
  },
  {
    type: "rescan",
    labelKey: "jobType.rescanLabel",
    descriptionKey: "jobType.rescanDesc",
    category: "scan",
    replayable: true,
    cards: INDEX_CARDS,
  },
  {
    type: "thumbnail_rebuild",
    labelKey: "jobType.thumbnail_rebuildLabel",
    descriptionKey: "jobType.thumbnail_rebuildDesc",
    category: "thumbnail",
    replayable: true,
    cards: THUMBNAIL_CARDS,
  },
  {
    type: "thumbnail_regenerate",
    labelKey: "jobType.thumbnail_regenerateLabel",
    descriptionKey: "jobType.thumbnail_regenerateDesc",
    category: "thumbnail",
    replayable: true,
    cards: THUMBNAIL_CARDS,
  },
  {
    type: "cbr_to_cbz",
    labelKey: "jobType.cbr_to_cbzLabel",
    descriptionKey: "jobType.cbr_to_cbzDesc",
    category: "conversion",
    replayable: false,
    cards: INDEX_CARDS,
  },
  {
    type: "metadata_batch",
    labelKey: "jobType.metadata_batchLabel",
    descriptionKey: "jobType.metadata_batchDesc",
    category: "metadata",
    replayable: true,
    cards: reportCards("metadata_batch", "jobDetail.metadataSearch", "jobDetail.metadataSearchDesc"),
  },
  {
    type: "metadata_batch_rematch",
    labelKey: "jobType.metadata_batch_rematchLabel",
    descriptionKey: "jobType.metadata_batch_rematchDesc",
    category: "metadata",
    replayable: true,
    cards: reportCards("metadata_batch", "jobDetail.metadataSearch", "jobDetail.metadataSearchDesc"),
  },
  {
    type: "metadata_refresh",
    labelKey: "jobType.metadata_refreshLabel",
    descriptionKey: "jobType.metadata_refreshDesc",
    category: "metadata",
    replayable: true,
    cards: reportCards("metadata_refresh", "jobDetail.metadataRefresh", "jobDetail.metadataRefreshDesc"),
  },
  {
    type: "metadata_refresh_all",
    labelKey: "jobType.metadata_refresh_allLabel",
    descriptionKey: "jobType.metadata_refresh_allDesc",
    category: "metadata",
    replayable: true,
    cards: reportCards("metadata_refresh", "jobDetail.metadataRefresh", "jobDetail.metadataRefreshDesc"),
  },
  {
    type: "reading_status_match",
    labelKey: "jobType.reading_status_matchLabel",
    descriptionKey: "jobType.reading_status_matchDesc",
    category: "reading",
    replayable: true,
    cards: reportCards(
      "reading_status_match",
      "jobDetail.readingStatusMatch",
      "jobDetail.readingStatusMatchDesc"
    ),
  },
  {
    type: "reading_status_push",
    labelKey: "jobType.reading_status_pushLabel",
    descriptionKey: "jobType.reading_status_pushDesc",
    category: "reading",
    replayable: true,
    cards: reportCards(
      "reading_status_push",
      "jobDetail.readingStatusPush",
      "jobDetail.readingStatusPushDesc"
    ),
  },
  {
    type: "rating_pull",
    labelKey: "jobType.rating_pullLabel",
    descriptionKey: "jobType.rating_pullDesc",
    category: "reading",
    replayable: true,
    cards: INDEX_CARDS,
  },
  {
    type: "download_detection",
    labelKey: "jobType.download_detectionLabel",
    descriptionKey: "jobType.download_detectionDesc",
    category: "download",
    replayable: true,
    cards: reportCards(
      "download_detection",
      "jobDetail.downloadDetection",
      "jobDetail.downloadDetectionDesc"
    ),
  },
  {
    type: "prowlarr_rss",
    labelKey: "jobType.prowlarr_rssLabel",
    descriptionKey: "jobType.prowlarr_rssDesc",
    category: "download",
    replayable: true,
    cards: reportCards("rss_poll", "jobType.prowlarr_rss", "jobType.prowlarr_rssDesc"),
  },
  {
    type: "telegram_sync",
    labelKey: "jobType.telegram_syncLabel",
    descriptionKey: "jobType.telegram_syncDesc",
    category: "download",
    replayable: false,
    cards: {
      kind: "telegram_sync",
      showProgress: false,
      showIndexStats: false,
      progress: { mode: "status", titleKey: "jobType.telegram_sync" },
    },
  },
  {
    type: "telegram_sync_incremental",
    labelKey: "jobType.telegram_sync_incrementalLabel",
    descriptionKey: "jobType.telegram_sync_incrementalDesc",
    category: "download",
    replayable: false,
    cards: {
      kind: "telegram_sync_incremental",
      showIndexStats: false,
      progress: { mode: "status", titleKey: "jobType.telegram_sync_incremental" },
    },
  },
];

/** Every type held by the registry, in declaration order. */
export const JOB_TYPES: readonly string[] = JOB_TYPE_SPECS.map((spec) => spec.type);

const DEFAULT_CARDS: CardPlanSpec = { kind: null, progress: STATUS_PROGRESS };

function resolveProgress(spec: ProgressSpec, t: TranslateFunction): JobProgressCopy {
  if (spec.mode === "fixed") {
    return { mode: "fixed", title: t(spec.titleKey), description: t(spec.descriptionKey) };
  }
  if (spec.mode === "title") {
    return { mode: "title", title: t(spec.titleKey) };
  }
  return spec.titleKey ? { mode: "status", title: t(spec.titleKey) } : { mode: "status" };
}

function resolveCards(spec: CardPlanSpec, t: TranslateFunction): JobResultCardPlan {
  return {
    kind: spec.kind,
    showProgress: spec.showProgress ?? true,
    showIndexStats: spec.showIndexStats ?? true,
    thumbnailOnly: spec.thumbnailOnly ?? false,
    progress: resolveProgress(spec.progress, t),
  };
}

/** Normalize a catalog category, falling back to `other` for unknown values. */
export function normalizeJobCategory(category: string | undefined | null): JobCategory {
  return (JOB_CATEGORIES as readonly string[]).includes(category ?? "")
    ? (category as JobCategory)
    : "other";
}

export interface JobDescriptorContext {
  t: TranslateFunction;
  /** Server job-type catalog; authoritative for `category` and `replayable`. */
  catalog?: readonly JobTypeCatalogDto[];
  /** Report fetchers, supplied by server components (see `lib/jobs/serverReports.ts`). */
  loaders?: JobReportLoaders;
}

/**
 * Resolve one job type: registry presentation + catalog facts.
 *
 * Unknown types keep the historic fallback — the raw type as label, no
 * description, `other` category, not replayable — so a job created by a newer
 * API still renders.
 */
export function resolveJobTypeDescriptor(
  type: string,
  { t, catalog, loaders }: JobDescriptorContext
): JobTypeDescriptor {
  const spec = JOB_TYPE_SPECS.find((entry) => entry.type === type);
  const catalogEntry = catalog?.find((entry) => entry.job_type === type);
  const kind = spec?.cards.kind ?? null;
  const load = kind && isJobReportKind(kind) && loaders ? loaders[kind] : null;

  return {
    type,
    label: spec ? t(spec.labelKey) : type,
    description: spec ? t(spec.descriptionKey) : "",
    category: normalizeJobCategory(catalogEntry?.category ?? spec?.category),
    replayable: catalogEntry?.replayable ?? spec?.replayable ?? false,
    report: load ? { load: (jobId: string) => load(jobId) } : null,
    resultCard: resolveCards(spec?.cards ?? DEFAULT_CARDS, t),
  };
}

/** Resolve the whole registry (used by tests and catalogue-wide UIs). */
export function jobTypeDescriptors(context: JobDescriptorContext): JobTypeDescriptor[] {
  return JOB_TYPE_SPECS.map((spec) => resolveJobTypeDescriptor(spec.type, context));
}

/**
 * Short label used by badges and the jobs-list filter. Always the
 * `jobType.<type>` key, so it stays shorter than the detail-page label.
 */
export function jobTypeShortLabel(type: string, t: TranslateFunction): string {
  const key = `jobType.${type}`;
  const translated = t(key as TranslationKey);
  return translated === key ? type : translated;
}

/**
 * Progress card copy for the current status: phase 1/2a/2b for status-driven
 * types, the fixed copy for report and thumbnail jobs.
 */
export function resolveJobProgressCopy(
  descriptor: JobTypeDescriptor,
  status: string,
  t: TranslateFunction
): { title: string; description?: string } {
  const copy = descriptor.resultCard.progress;
  if (copy.mode === "title") return { title: copy.title };
  if (copy.mode === "fixed") return { title: copy.title, description: copy.description };
  if (status === "extracting_pages") {
    return { title: copy.title ?? t("jobDetail.phase2a"), description: t("jobDetail.phase2aDesc") };
  }
  if (status === "generating_thumbnails") {
    return { title: copy.title ?? t("jobDetail.phase2b"), description: t("jobDetail.phase2bDesc") };
  }
  return { title: copy.title ?? t("jobDetail.phase1"), description: t("jobDetail.phase1Desc") };
}

/** Narrow a loaded report to one card family. */
export function jobReportOf<K extends JobReportKind>(
  report: JobReport | null,
  kind: K
): Extract<JobReport, { kind: K }> | null {
  return report && report.kind === kind ? (report as Extract<JobReport, { kind: K }>) : null;
}

/** Per-family report props of `JobSummaryBanner`, extracted from one load. */
export function jobSummaryReports(report: JobReport | null) {
  const detection = jobReportOf(report, "download_detection") ?? jobReportOf(report, "rss_poll");
  return {
    batchReport: jobReportOf(report, "metadata_batch")?.report ?? null,
    refreshReport: jobReportOf(report, "metadata_refresh")?.report ?? null,
    readingStatusReport: jobReportOf(report, "reading_status_match")?.report ?? null,
    readingStatusPushReport: jobReportOf(report, "reading_status_push")?.report ?? null,
    downloadDetectionReport: detection?.report ?? null,
  };
}

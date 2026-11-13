import { describe, expect, it, vi } from "vitest";

import type { JobTypeCatalogDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import {
  JOB_CATEGORIES,
  JOB_TYPES,
  jobSummaryReports,
  jobTypeDescriptors,
  jobTypeShortLabel,
  normalizeJobCategory,
  resolveJobProgressCopy,
  resolveJobTypeDescriptor,
  type JobReport,
  type JobReportKind,
  type JobReportLoaders,
} from "@/lib/jobs/descriptors";

const t = ((key: string) => key) as unknown as TranslateFunction;

function catalogEntry(
  jobType: string,
  overrides: Partial<JobTypeCatalogDto> = {}
): JobTypeCatalogDto {
  return {
    job_type: jobType,
    owner: "indexer",
    category: "scan",
    claimable: true,
    replayable: true,
    requires_library: false,
    ...overrides,
  };
}

function fakeLoaders(): { loaders: JobReportLoaders; calls: JobReportKind[] } {
  const calls: JobReportKind[] = [];
  const make = (kind: JobReportKind) => async () => {
    calls.push(kind);
    return { kind, report: null, results: [], errors: [] } as unknown as JobReport;
  };
  return {
    calls,
    loaders: {
      metadata_batch: make("metadata_batch"),
      metadata_refresh: make("metadata_refresh"),
      reading_status_match: make("reading_status_match"),
      reading_status_push: make("reading_status_push"),
      download_detection: make("download_detection"),
      rss_poll: make("rss_poll"),
    },
  };
}

describe("job type registry", () => {
  it("covers the seventeen backoffice job types exactly once", () => {
    expect(JOB_TYPES).toHaveLength(17);
    expect(new Set(JOB_TYPES).size).toBe(17);
    expect(JOB_TYPES).toEqual(
      expect.arrayContaining([
        "rebuild",
        "full_rebuild",
        "rescan",
        "thumbnail_rebuild",
        "thumbnail_regenerate",
        "cbr_to_cbz",
        "metadata_batch",
        "metadata_batch_rematch",
        "metadata_refresh",
        "metadata_refresh_all",
        "reading_status_match",
        "reading_status_push",
        "rating_pull",
        "download_detection",
        "prowlarr_rss",
        "telegram_sync",
        "telegram_sync_incremental",
      ])
    );
  });

  it("resolves a label and a description for every registered type", () => {
    for (const descriptor of jobTypeDescriptors({ t })) {
      expect(descriptor.label).toBe(`jobType.${descriptor.type}Label`);
      expect(descriptor.description).toBe(`jobType.${descriptor.type}Desc`);
      expect(JOB_CATEGORIES).toContain(descriptor.category);
    }
  });

  it("defaults categories and replayability from the registry when the catalog is missing", () => {
    const rescan = resolveJobTypeDescriptor("rescan", { t });
    expect(rescan.category).toBe("scan");
    expect(rescan.replayable).toBe(true);

    const cbr = resolveJobTypeDescriptor("cbr_to_cbz", { t });
    expect(cbr.category).toBe("conversion");
    expect(cbr.replayable).toBe(false);

    const telegram = resolveJobTypeDescriptor("telegram_sync", { t });
    expect(telegram.replayable).toBe(false);
  });

  it("lets the server catalog override category and replayability", () => {
    const descriptor = resolveJobTypeDescriptor("rating_pull", {
      t,
      catalog: [catalogEntry("rating_pull", { category: "search", replayable: false })],
    });

    expect(descriptor.category).toBe("search");
    expect(descriptor.replayable).toBe(false);
    // Presentation still comes from the registry.
    expect(descriptor.label).toBe("jobType.rating_pullLabel");
  });

  it("renders catalog-only types without a registry entry", () => {
    const descriptor = resolveJobTypeDescriptor("ddl_search", {
      t,
      catalog: [catalogEntry("ddl_search", { category: "search", replayable: true })],
    });

    expect(descriptor.label).toBe("ddl_search");
    expect(descriptor.description).toBe("");
    expect(descriptor.category).toBe("search");
    expect(descriptor.replayable).toBe(true);
    expect(descriptor.resultCard.kind).toBeNull();
  });

  it("normalizes unknown categories to `other`", () => {
    expect(normalizeJobCategory("scan")).toBe("scan");
    expect(normalizeJobCategory("nonsense")).toBe("other");
    expect(normalizeJobCategory(undefined)).toBe("other");
    expect(normalizeJobCategory(null)).toBe("other");
  });

  it("falls back safely for an unknown type with no catalog", () => {
    const descriptor = resolveJobTypeDescriptor("brand_new_job", { t });

    expect(descriptor).toMatchObject({
      type: "brand_new_job",
      label: "brand_new_job",
      description: "",
      category: "other",
      replayable: false,
      report: null,
    });
    expect(descriptor.resultCard).toEqual({
      kind: null,
      showProgress: true,
      showIndexStats: true,
      thumbnailOnly: false,
      progress: { mode: "status" },
    });
  });
});

describe("job result cards", () => {
  it("plans the index, thumbnail, report and telegram families", () => {
    const rebuild = resolveJobTypeDescriptor("rebuild", { t });
    expect(rebuild.resultCard).toMatchObject({ kind: null, showProgress: true, showIndexStats: true });

    const thumbnail = resolveJobTypeDescriptor("thumbnail_rebuild", { t });
    expect(thumbnail.resultCard).toMatchObject({
      kind: null,
      showIndexStats: false,
      thumbnailOnly: true,
      progress: { mode: "title", title: "jobType.thumbnail_rebuild" },
    });

    const batch = resolveJobTypeDescriptor("metadata_batch_rematch", { t });
    expect(batch.resultCard).toMatchObject({ kind: "metadata_batch", showIndexStats: false });

    const refreshAll = resolveJobTypeDescriptor("metadata_refresh_all", { t });
    expect(refreshAll.resultCard.kind).toBe("metadata_refresh");

    const rss = resolveJobTypeDescriptor("prowlarr_rss", { t });
    expect(rss.resultCard.kind).toBe("rss_poll");

    const telegram = resolveJobTypeDescriptor("telegram_sync", { t });
    expect(telegram.resultCard).toMatchObject({ kind: "telegram_sync", showProgress: false });

    const incremental = resolveJobTypeDescriptor("telegram_sync_incremental", { t });
    expect(incremental.resultCard).toMatchObject({
      kind: "telegram_sync_incremental",
      showProgress: true,
      showIndexStats: false,
    });
  });
});

describe("report wiring", () => {
  it("exposes a loader for every report-backed type and none for the others", () => {
    const { loaders } = fakeLoaders();

    expect(resolveJobTypeDescriptor("metadata_batch", { t, loaders }).report).not.toBeNull();
    expect(resolveJobTypeDescriptor("metadata_refresh", { t, loaders }).report).not.toBeNull();
    expect(resolveJobTypeDescriptor("reading_status_match", { t, loaders }).report).not.toBeNull();
    expect(resolveJobTypeDescriptor("reading_status_push", { t, loaders }).report).not.toBeNull();
    expect(resolveJobTypeDescriptor("download_detection", { t, loaders }).report).not.toBeNull();
    expect(resolveJobTypeDescriptor("prowlarr_rss", { t, loaders }).report).not.toBeNull();

    expect(resolveJobTypeDescriptor("rebuild", { t, loaders }).report).toBeNull();
    expect(resolveJobTypeDescriptor("thumbnail_rebuild", { t, loaders }).report).toBeNull();
    expect(resolveJobTypeDescriptor("telegram_sync", { t, loaders }).report).toBeNull();
    expect(resolveJobTypeDescriptor("telegram_sync_incremental", { t, loaders }).report).toBeNull();
  });

  it("without injected loaders no report fetch is planned", () => {
    expect(resolveJobTypeDescriptor("metadata_batch", { t }).report).toBeNull();
  });

  it("parses fixed progress copy only for report jobs", () => {
    const { loaders } = fakeLoaders();
    const batch = resolveJobTypeDescriptor("metadata_batch", { t, loaders });
    expect(batch.resultCard.progress).toEqual({
      mode: "fixed",
      title: "jobDetail.metadataSearch",
      description: "jobDetail.metadataSearchDesc",
    });

    const detection = resolveJobTypeDescriptor("download_detection", { t, loaders });
    expect(detection.resultCard.progress).toEqual({
      mode: "fixed",
      title: "jobDetail.downloadDetection",
      description: "jobDetail.downloadDetectionDesc",
    });

    const rss = resolveJobTypeDescriptor("prowlarr_rss", { t, loaders });
    expect(rss.resultCard.progress).toEqual({
      mode: "fixed",
      title: "jobType.prowlarr_rss",
      description: "jobType.prowlarr_rssDesc",
    });
  });
});

describe("resolveJobProgressCopy", () => {
  const index = resolveJobTypeDescriptor("rebuild", { t });

  it("follows the running phase for status-driven types", () => {
    expect(resolveJobProgressCopy(index, "pending", t)).toEqual({
      title: "jobDetail.phase1",
      description: "jobDetail.phase1Desc",
    });
    expect(resolveJobProgressCopy(index, "extracting_pages", t)).toEqual({
      title: "jobDetail.phase2a",
      description: "jobDetail.phase2aDesc",
    });
    expect(resolveJobProgressCopy(index, "generating_thumbnails", t)).toEqual({
      title: "jobDetail.phase2b",
      description: "jobDetail.phase2bDesc",
    });
  });

  it("keeps a fixed title while the description follows the phase", () => {
    const telegram = resolveJobTypeDescriptor("telegram_sync", { t });
    expect(resolveJobProgressCopy(telegram, "running", t)).toEqual({
      title: "jobType.telegram_sync",
      description: "jobDetail.phase1Desc",
    });
    expect(resolveJobProgressCopy(telegram, "generating_thumbnails", t)).toEqual({
      title: "jobType.telegram_sync",
      description: "jobDetail.phase2bDesc",
    });
  });

  it("returns no description for thumbnail jobs", () => {
    const thumbnail = resolveJobTypeDescriptor("thumbnail_rebuild", { t });
    expect(resolveJobProgressCopy(thumbnail, "running", t)).toEqual({
      title: "jobType.thumbnail_rebuild",
    });
  });

  it("returns the fixed copy for report jobs", () => {
    const batch = resolveJobTypeDescriptor("metadata_batch", { t });
    expect(resolveJobProgressCopy(batch, "running", t)).toEqual({
      title: "jobDetail.metadataSearch",
      description: "jobDetail.metadataSearchDesc",
    });
  });
});

describe("jobSummaryReports", () => {
  it("extracts every report family and folds RSS into detection", () => {
    const summary = jobSummaryReports({
      kind: "rss_poll",
      report: { job_id: "job-1" } as never,
      results: [],
      errors: [],
    });

    expect(summary.downloadDetectionReport).toEqual({ job_id: "job-1" });
    expect(summary.batchReport).toBeNull();
    expect(summary.refreshReport).toBeNull();
    expect(summary.readingStatusReport).toBeNull();
    expect(summary.readingStatusPushReport).toBeNull();
  });

  it("returns empty props for a missing report", () => {
    expect(jobSummaryReports(null)).toEqual({
      batchReport: null,
      refreshReport: null,
      readingStatusReport: null,
      readingStatusPushReport: null,
      downloadDetectionReport: null,
    });
  });
});

describe("jobTypeShortLabel", () => {
  const shortT = vi.fn((key: string) => (key === "jobType.rebuild" ? "Indexation" : key));

  it("uses the short jobType key", () => {
    expect(jobTypeShortLabel("rebuild", shortT as unknown as TranslateFunction)).toBe("Indexation");
  });

  it("falls back to the raw type when the key is missing", () => {
    expect(jobTypeShortLabel("unknown_type", shortT as unknown as TranslateFunction)).toBe(
      "unknown_type"
    );
  });
});

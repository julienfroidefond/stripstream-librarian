/**
 * Server-side report loaders for the job descriptors.
 *
 * `lib/jobs/descriptors.ts` must stay importable by Client Components, so it
 * never imports the API runtime (only DTO types). The actual fetchers live here
 * and are injected by server components such as the job detail page.
 */

import {
  getDownloadDetectionReport,
  getDownloadDetectionResults,
  getMetadataBatchReport,
  getMetadataBatchResults,
  getMetadataRefreshReport,
  getReadingStatusMatchReport,
  getReadingStatusMatchResults,
  getReadingStatusPushReport,
  getReadingStatusPushResults,
} from "@/lib/api";

import type { JobReport, JobReportLoaders } from "./descriptors";

async function loadMetadataBatch(jobId: string): Promise<JobReport> {
  const [report, results] = await Promise.all([
    getMetadataBatchReport(jobId).catch(() => null),
    getMetadataBatchResults(jobId).catch(() => []),
  ]);
  return { kind: "metadata_batch", report, results };
}

async function loadMetadataRefresh(jobId: string): Promise<JobReport> {
  const report = await getMetadataRefreshReport(jobId).catch(() => null);
  return { kind: "metadata_refresh", report };
}

async function loadReadingStatusMatch(jobId: string): Promise<JobReport> {
  const [report, results] = await Promise.all([
    getReadingStatusMatchReport(jobId).catch(() => null),
    getReadingStatusMatchResults(jobId).catch(() => []),
  ]);
  return { kind: "reading_status_match", report, results };
}

async function loadReadingStatusPush(jobId: string): Promise<JobReport> {
  const [report, results] = await Promise.all([
    getReadingStatusPushReport(jobId).catch(() => null),
    getReadingStatusPushResults(jobId).catch(() => []),
  ]);
  return { kind: "reading_status_push", report, results };
}

/** Detection and Prowlarr RSS share the same endpoints. */
async function loadDetection(jobId: string, kind: "download_detection" | "rss_poll"): Promise<JobReport> {
  const [report, results, errors] = await Promise.all([
    getDownloadDetectionReport(jobId).catch(() => null),
    getDownloadDetectionResults(jobId, "found").catch(() => []),
    getDownloadDetectionResults(jobId, "error").catch(() => []),
  ]);
  return { kind, report, results, errors };
}

export const jobReportLoaders: JobReportLoaders = {
  metadata_batch: loadMetadataBatch,
  metadata_refresh: loadMetadataRefresh,
  reading_status_match: loadReadingStatusMatch,
  reading_status_push: loadReadingStatusPush,
  download_detection: (jobId) => loadDetection(jobId, "download_detection"),
  rss_poll: (jobId) => loadDetection(jobId, "rss_poll"),
};

"use server";

import { invalidateAfter } from "@/lib/cache";
import { ApiError } from "@/lib/api/errors";
import { redirect } from "next/navigation";

import {
  rebuildIndex,
  rebuildThumbnails,
  regenerateThumbnails,
  startDdlSearchAll,
  startDownloadDetection,
  startMetadataBatch,
  startMetadataRefresh,
  startMetadataRefreshAll,
  startReadingStatusMatch,
  startReadingStatusPull,
  startReadingStatusPush,
  startRssPoll,
  startTelegramSync,
  startTelegramSyncIncremental,
} from "@/lib/api";

interface JobResult {
  id?: string | null;
}

type LibraryJobRunner = (libraryId: string | undefined) => Promise<JobResult>;

function getLibraryIds(formData: FormData): string[] {
  return formData
    .getAll("library_id")
    .map((value) => (typeof value === "string" ? value.trim() : ""))
    .filter((value) => value.length > 0);
}

function jobRedirect(resultId: string | undefined | null, libraryIds: string[]): never {
  const params = new URLSearchParams();
  if (resultId) params.set("highlight", resultId);
  for (const libraryId of libraryIds) params.append("library", libraryId);
  const qs = params.toString();
  redirect(qs ? `/jobs?${qs}` : "/jobs");
}

/** The upstream `error` field of an API failure, else the error's own message. */
function errorMessage(error: unknown): string {
  if (error instanceof ApiError && error.detail) return error.detail;
  if (error instanceof Error) return error.message;
  return "An error occurred";
}

function errorRedirect(error: unknown): never {
  redirect(`/jobs?error=${encodeURIComponent(errorMessage(error))}`);
}

function isNextNavigationError(error: unknown): boolean {
  return Boolean(error && typeof error === "object" && "digest" in error);
}

/**
 * Runs a job launch and turns any failure into a redirect to `/jobs?error=`.
 * Next's own navigation signal (the success `redirect`) is rethrown as is.
 */
async function redirectOnError(launch: () => Promise<never>): Promise<never> {
  try {
    return await launch();
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

/**
 * Runs the action once per selected library.
 * No selection ids means "all libraries": the API handles the fan-out itself.
 */
function runPerLibrary(formData: FormData, run: LibraryJobRunner): Promise<never> {
  return redirectOnError(async () => {
    const libraryIds = getLibraryIds(formData);
    const targets: Array<string | undefined> = libraryIds.length > 0 ? libraryIds : [undefined];
    const results: JobResult[] = [];
    for (const libraryId of targets) {
      results.push(await run(libraryId));
    }
    invalidateAfter({ type: "jobs.started" });
    const highlightId = results.find((result) => result.id)?.id ?? undefined;
    jobRedirect(highlightId, libraryIds);
  });
}

function runGlobal(run: () => Promise<JobResult>): Promise<never> {
  return redirectOnError(async () => {
    const result = await run();
    invalidateAfter({ type: "jobs.started" });
    jobRedirect(result.id, []);
  });
}

export async function triggerRebuild(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => rebuildIndex(libraryId));
}

export async function triggerFullRebuild(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => rebuildIndex(libraryId, true));
}

export async function triggerRescan(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => rebuildIndex(libraryId, false, true));
}

export async function triggerThumbnailsRebuild(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => rebuildThumbnails(libraryId));
}

export async function triggerThumbnailsRegenerate(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => regenerateThumbnails(libraryId));
}

export async function triggerMetadataBatch(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startMetadataBatch(libraryId));
}

export async function triggerMetadataRematch(formData: FormData) {
  if (getLibraryIds(formData).length === 0) {
    return redirect("/jobs");
  }
  await runPerLibrary(formData, (libraryId) => startMetadataBatch(libraryId, true));
}

export async function triggerMetadataRefresh(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startMetadataRefresh(libraryId));
}

export async function triggerMetadataRefreshAll(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startMetadataRefreshAll(libraryId));
}

export async function triggerReadingStatusMatch(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startReadingStatusMatch(libraryId));
}

export async function triggerReadingStatusPush(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startReadingStatusPush(libraryId));
}

export async function triggerRatingPull(_formData: FormData) {
  await runGlobal(() => startReadingStatusPull());
}

export async function triggerDownloadDetection(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startDownloadDetection(libraryId));
}

export async function triggerRssPoll(formData: FormData) {
  await runPerLibrary(formData, (libraryId) => startRssPoll(libraryId));
}

export async function triggerTelegramSync(_formData: FormData) {
  await runGlobal(() => startTelegramSync());
}

export async function triggerTelegramSyncIncremental(_formData: FormData) {
  await runGlobal(() => startTelegramSyncIncremental());
}

export async function triggerDdlSearch(_formData: FormData) {
  await runGlobal(() => startDdlSearchAll());
}

"use server";

import { revalidatePath } from "next/cache";
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

function errorRedirect(error: unknown): never {
  const msg = error instanceof Error ? error.message : "An error occurred";
  // Extract the API error message if present (format: "API /path failed (400): message")
  const match = msg.match(/\(\d+\):\s*(.+)/);
  const cleanMsg = match ? match[1] : msg;
  redirect(`/jobs?error=${encodeURIComponent(cleanMsg)}`);
}

function isNextNavigationError(error: unknown): boolean {
  return Boolean(error && typeof error === "object" && "digest" in error);
}

/**
 * Runs the action once per selected library.
 * No selection ids means "all libraries": the API handles the fan-out itself.
 */
async function runPerLibrary(formData: FormData, run: LibraryJobRunner): Promise<never> {
  const libraryIds = getLibraryIds(formData);
  const targets: Array<string | undefined> = libraryIds.length > 0 ? libraryIds : [undefined];
  const results: JobResult[] = [];
  for (const libraryId of targets) {
    results.push(await run(libraryId));
  }
  revalidatePath("/jobs");
  const highlightId = results.find((result) => result.id)?.id ?? undefined;
  jobRedirect(highlightId, libraryIds);
}

async function runGlobal(run: () => Promise<JobResult>): Promise<never> {
  try {
    const result = await run();
    revalidatePath("/jobs");
    jobRedirect(result.id, []);
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerRebuild(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => rebuildIndex(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerFullRebuild(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => rebuildIndex(libraryId, true));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerRescan(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => rebuildIndex(libraryId, false, true));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerThumbnailsRebuild(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => rebuildThumbnails(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerThumbnailsRegenerate(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => regenerateThumbnails(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerMetadataBatch(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startMetadataBatch(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerMetadataRematch(formData: FormData) {
  if (getLibraryIds(formData).length === 0) {
    return redirect("/jobs");
  }
  try {
    await runPerLibrary(formData, (libraryId) => startMetadataBatch(libraryId, true));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerMetadataRefresh(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startMetadataRefresh(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerMetadataRefreshAll(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startMetadataRefreshAll(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerReadingStatusMatch(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startReadingStatusMatch(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerReadingStatusPush(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startReadingStatusPush(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerRatingPull(_formData: FormData) {
  await runGlobal(() => startReadingStatusPull());
}

export async function triggerDownloadDetection(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startDownloadDetection(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
}

export async function triggerRssPoll(formData: FormData) {
  try {
    await runPerLibrary(formData, (libraryId) => startRssPoll(libraryId));
  } catch (e) {
    if (isNextNavigationError(e)) throw e;
    errorRedirect(e);
  }
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

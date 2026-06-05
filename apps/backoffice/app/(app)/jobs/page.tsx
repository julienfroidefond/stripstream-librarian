import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listJobs, fetchLibraries, rebuildIndex, rebuildThumbnails, regenerateThumbnails, startMetadataBatch, startMetadataRefresh, startMetadataRefreshAll, startReadingStatusMatch, startReadingStatusPush, startDownloadDetection, startRssPoll, fetchDownloadsEnabled, startTelegramSync, startTelegramSyncIncremental, fetchTelegramAuthorized, IndexJobDto, LibraryDto } from "@/lib/api";
import { JobsList } from "@/app/components/JobsList";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/app/components/ui";
import { LibraryBadgeSelector } from "./components/LibraryBadgeSelector";
import { getServerTranslations } from "@/lib/i18n/server";

export const dynamic = "force-dynamic";

function jobRedirect(resultId: string | undefined | null, libraryId: string): never {
  const params = new URLSearchParams();
  if (resultId) params.set("highlight", resultId);
  if (libraryId) params.set("library", libraryId);
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

export default async function JobsPage({ searchParams }: { searchParams: Promise<{ highlight?: string; library?: string; error?: string }> }) {
  const { highlight, library, error: errorMsg } = await searchParams;
  const { t } = await getServerTranslations();
  const [jobs, libraries, prowlarrConfigured, telegramAuthorized] = await Promise.all([
    listJobs().catch(() => [] as IndexJobDto[]),
    fetchLibraries().catch(() => [] as LibraryDto[]),
    fetchDownloadsEnabled(),
    fetchTelegramAuthorized(),
  ]);

  const libraryMap = new Map(libraries.map(l => [l.id, l.name]));
  const readingStatusLibraries = libraries.filter(l => l.reading_status_provider);

  async function triggerRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await rebuildIndex(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerFullRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await rebuildIndex(libraryId || undefined, true); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerRescan(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await rebuildIndex(libraryId || undefined, false, true); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerThumbnailsRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await rebuildThumbnails(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerThumbnailsRegenerate(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await regenerateThumbnails(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerMetadataBatch(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startMetadataBatch(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerMetadataRematch(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    if (!libraryId) return redirect("/jobs");
    try { const result = await startMetadataBatch(libraryId, true); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerMetadataRefresh(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startMetadataRefresh(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerMetadataRefreshAll(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startMetadataRefreshAll(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerReadingStatusMatch(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startReadingStatusMatch(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerReadingStatusPush(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startReadingStatusPush(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerDownloadDetection(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startDownloadDetection(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerRssPoll(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    try { const result = await startRssPoll(libraryId || undefined); revalidatePath("/jobs"); jobRedirect(result.id ?? undefined, libraryId); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerTelegramSync() {
    "use server";
    try { const result = await startTelegramSync(); revalidatePath("/jobs"); jobRedirect(result.id ?? undefined, ""); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  async function triggerTelegramSyncIncremental() {
    "use server";
    try { const result = await startTelegramSyncIncremental(); revalidatePath("/jobs"); jobRedirect(result.id ?? undefined, ""); }
    catch (e) { if (e && typeof e === "object" && "digest" in e) throw e; errorRedirect(e); }
  }

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          {t("jobs.title")}
        </h1>
      </div>

      {errorMsg && (
        <div className="mb-4 p-3 rounded-lg bg-destructive/10 border border-destructive/20 text-destructive text-sm">
          {errorMsg}
        </div>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>{t("jobs.startJob")}</CardTitle>
          <CardDescription>{t("jobs.startJobDescription")}</CardDescription>
        </CardHeader>
        <CardContent>
          <LibraryBadgeSelector libraries={libraries.map(l => ({ id: l.id, name: l.name }))} initialSelected={library ?? null}>
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">

              {/* Indexation group */}
              <div className="space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
                  <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                  {t("jobs.groupIndexation")}
                </div>
                <div className="space-y-2">
                  <button type="submit" formAction={triggerRebuild}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.rebuild")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.rebuildShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerRescan}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.rescan")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.rescanShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerFullRebuild}
                    className="w-full text-left rounded-lg border border-destructive/30 bg-destructive/5 p-3 hover:bg-destructive/10 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-destructive shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                      </svg>
                      <span className="font-medium text-sm text-destructive">{t("jobs.fullRebuild")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.fullRebuildShort")}</p>
                  </button>
                </div>
              </div>

              {/* Thumbnails group */}
              <div className="space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
                  <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                  </svg>
                  {t("jobs.groupThumbnails")}
                </div>
                <div className="space-y-2">
                  <button type="submit" formAction={triggerThumbnailsRebuild}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.generateThumbnails")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.generateThumbnailsShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerThumbnailsRegenerate}
                    className="w-full text-left rounded-lg border border-warning/30 bg-warning/5 p-3 hover:bg-warning/10 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-warning shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                      </svg>
                      <span className="font-medium text-sm text-warning">{t("jobs.regenerateThumbnails")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.regenerateThumbnailsShort")}</p>
                  </button>
                </div>
              </div>

              {/* Metadata group */}
              <div className="space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
                  <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
                  </svg>
                  {t("jobs.groupMetadata")}
                </div>
                <div className="space-y-2">
                  <button type="submit" formAction={triggerMetadataBatch}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-background">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.batchMetadata")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.batchMetadataShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerMetadataRematch}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-background">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-warning shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.rematchMetadata")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.rematchMetadataShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerMetadataRefresh}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-background">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.refreshMetadata")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.refreshMetadataShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerMetadataRefreshAll}
                    className="w-full text-left rounded-lg border border-warning/30 bg-warning/5 p-3 hover:bg-warning/10 transition-colors group cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-background">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-warning shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      <span className="font-medium text-sm text-warning">{t("jobs.refreshMetadataAll")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.refreshMetadataAllShort")}</p>
                  </button>
                </div>
              </div>

              {/* Reading status group — only shown if at least one library has a provider configured */}
              {readingStatusLibraries.length > 0 && (
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
                    <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    {t("jobs.groupReadingStatus")}
                  </div>
                  <div className="space-y-2">
                    <button type="submit" formAction={triggerReadingStatusMatch}
                      className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                      <div className="flex items-center gap-2">
                        <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                        </svg>
                        <span className="font-medium text-sm text-foreground">{t("jobs.matchReadingStatus")}</span>
                      </div>
                      <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.matchReadingStatusShort")}</p>
                    </button>
                    <button type="submit" formAction={triggerReadingStatusPush}
                      className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                      <div className="flex items-center gap-2">
                        <svg className="w-4 h-4 text-success shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                        </svg>
                        <span className="font-medium text-sm text-foreground">{t("jobs.pushReadingStatus")}</span>
                      </div>
                      <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.pushReadingStatusShort")}</p>
                    </button>
                  </div>
                </div>
              )}

              {/* Download group — only shown if Prowlarr is configured */}
              {prowlarrConfigured && <div className="space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
                  <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                  </svg>
                  {t("jobs.groupProwlarr")}
                </div>
                <div className="space-y-2">
                  <button type="submit" formAction={triggerDownloadDetection}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0zM10 7v3m0 0v3m0-3h3m-3 0H7" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.downloadDetection")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.downloadDetectionShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerRssPoll}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 5c7.18 0 13 5.82 13 13M6 11a7 7 0 017 7M6 17a1 1 0 110 2 1 1 0 010-2z" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.rssPoll")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.rssPollShort")}</p>
                  </button>
                </div>
              </div>}

              {/* Telegram group — only shown if Telegram is authorized */}
              {telegramAuthorized && <div className="space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
                  <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
                  </svg>
                  {t("jobs.groupTelegram")}
                </div>
                <div className="space-y-2">
                  <button type="submit" formAction={triggerTelegramSync}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.telegramSync")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.telegramSyncShort")}</p>
                  </button>
                  <button type="submit" formAction={triggerTelegramSyncIncremental}
                    className="w-full text-left rounded-lg border border-input bg-background p-3 hover:bg-accent/50 transition-colors group cursor-pointer">
                    <div className="flex items-center gap-2">
                      <svg className="w-4 h-4 text-primary shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                      </svg>
                      <span className="font-medium text-sm text-foreground">{t("jobs.telegramSyncIncremental")}</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1 ml-6">{t("jobs.telegramSyncIncrementalShort")}</p>
                  </button>
                </div>
              </div>}

            </div>
          </LibraryBadgeSelector>
        </CardContent>
      </Card>

      <JobsList
        initialJobs={jobs}
        libraries={libraryMap}
        highlightJobId={highlight}
      />
    </>
  );
}

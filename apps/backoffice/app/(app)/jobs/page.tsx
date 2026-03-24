import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listJobs, fetchLibraries, rebuildIndex, rebuildThumbnails, regenerateThumbnails, startMetadataBatch, startMetadataRefresh, IndexJobDto, LibraryDto } from "@/lib/api";
import { JobsList } from "@/app/components/JobsList";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormSelect } from "@/app/components/ui";
import { getServerTranslations } from "@/lib/i18n/server";

export const dynamic = "force-dynamic";

export default async function JobsPage({ searchParams }: { searchParams: Promise<{ highlight?: string }> }) {
  const { highlight } = await searchParams;
  const { t } = await getServerTranslations();
  const [jobs, libraries] = await Promise.all([
    listJobs().catch(() => [] as IndexJobDto[]),
    fetchLibraries().catch(() => [] as LibraryDto[])
  ]);

  const libraryMap = new Map(libraries.map(l => [l.id, l.name]));

  async function triggerRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    const result = await rebuildIndex(libraryId || undefined);
    revalidatePath("/jobs");
    redirect(`/jobs?highlight=${result.id}`);
  }

  async function triggerFullRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    const result = await rebuildIndex(libraryId || undefined, true);
    revalidatePath("/jobs");
    redirect(`/jobs?highlight=${result.id}`);
  }

  async function triggerRescan(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    const result = await rebuildIndex(libraryId || undefined, false, true);
    revalidatePath("/jobs");
    redirect(`/jobs?highlight=${result.id}`);
  }

  async function triggerThumbnailsRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    const result = await rebuildThumbnails(libraryId || undefined);
    revalidatePath("/jobs");
    redirect(`/jobs?highlight=${result.id}`);
  }

  async function triggerThumbnailsRegenerate(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    const result = await regenerateThumbnails(libraryId || undefined);
    revalidatePath("/jobs");
    redirect(`/jobs?highlight=${result.id}`);
  }

  async function triggerMetadataBatch(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    if (libraryId) {
      let result;
      try {
        result = await startMetadataBatch(libraryId);
      } catch {
        // Library may have metadata disabled — ignore silently
        return;
      }
      revalidatePath("/jobs");
      redirect(`/jobs?highlight=${result.id}`);
    } else {
      // All libraries — skip those with metadata disabled
      const allLibraries = await fetchLibraries().catch(() => [] as LibraryDto[]);
      let lastId: string | undefined;
      for (const lib of allLibraries) {
        if (lib.metadata_provider === "none") continue;
        try {
          const result = await startMetadataBatch(lib.id);
          if (result.status !== "already_running") lastId = result.id;
        } catch {
          // Library may have metadata disabled or other issue — skip
        }
      }
      revalidatePath("/jobs");
      redirect(lastId ? `/jobs?highlight=${lastId}` : "/jobs");
    }
  }

  async function triggerMetadataRefresh(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    if (libraryId) {
      let result;
      try {
        result = await startMetadataRefresh(libraryId);
      } catch {
        return;
      }
      revalidatePath("/jobs");
      redirect(`/jobs?highlight=${result.id}`);
    } else {
      // All libraries — skip those with metadata disabled
      const allLibraries = await fetchLibraries().catch(() => [] as LibraryDto[]);
      let lastId: string | undefined;
      for (const lib of allLibraries) {
        if (lib.metadata_provider === "none") continue;
        try {
          const result = await startMetadataRefresh(lib.id);
          if (result.status !== "already_running") lastId = result.id;
        } catch {
          // Library may have metadata disabled or no approved links — skip
        }
      }
      revalidatePath("/jobs");
      redirect(lastId ? `/jobs?highlight=${lastId}` : "/jobs");
    }
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

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>{t("jobs.startJob")}</CardTitle>
          <CardDescription>{t("jobs.startJobDescription")}</CardDescription>
        </CardHeader>
        <CardContent>
          <form>
            <div className="mb-6">
              <FormField className="max-w-xs">
                <FormSelect name="library_id" defaultValue="">
                  <option value="">{t("jobs.allLibraries")}</option>
                  {libraries.map((lib) => (
                    <option key={lib.id} value={lib.id}>{lib.name}</option>
                  ))}
                </FormSelect>
              </FormField>
            </div>
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
                </div>
              </div>

            </div>
          </form>
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

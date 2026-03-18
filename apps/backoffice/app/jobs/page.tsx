import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listJobs, fetchLibraries, rebuildIndex, rebuildThumbnails, regenerateThumbnails, startMetadataBatch, IndexJobDto, LibraryDto } from "../../lib/api";
import { JobsList } from "../components/JobsList";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormSelect, FormRow } from "../components/ui";
import { getServerTranslations } from "../../lib/i18n/server";

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
    if (!libraryId) return;
    const result = await startMetadataBatch(libraryId);
    revalidatePath("/jobs");
    redirect(`/jobs?highlight=${result.id}`);
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
            <FormRow>
              <FormField className="flex-1 max-w-xs">
                <FormSelect name="library_id" defaultValue="">
                  <option value="">{t("jobs.allLibraries")}</option>
                  {libraries.map((lib) => (
                    <option key={lib.id} value={lib.id}>{lib.name}</option>
                  ))}
                </FormSelect>
              </FormField>
              <div className="flex flex-wrap gap-2">
                <Button type="submit" formAction={triggerRebuild}>
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
                  {t("jobs.rebuild")}
                </Button>
                <Button type="submit" formAction={triggerFullRebuild} variant="warning">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                  {t("jobs.fullRebuild")}
                </Button>
                <Button type="submit" formAction={triggerThumbnailsRebuild} variant="secondary">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                  </svg>
                  {t("jobs.generateThumbnails")}
                </Button>
                <Button type="submit" formAction={triggerThumbnailsRegenerate} variant="warning">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
                  {t("jobs.regenerateThumbnails")}
                </Button>
                <Button type="submit" formAction={triggerMetadataBatch} variant="secondary">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                  {t("jobs.batchMetadata")}
                </Button>
              </div>
            </FormRow>
          </form>
        </CardContent>
      </Card>

      {/* Job types legend */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">{t("jobs.referenceTitle")}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
            <div className="flex gap-3">
              <div className="shrink-0 mt-0.5">
                <svg className="w-5 h-5 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
              </div>
              <div>
                <span className="font-medium text-foreground">{t("jobs.rebuild")}</span>
                <p className="text-muted-foreground text-xs mt-0.5" dangerouslySetInnerHTML={{ __html: t("jobs.rebuildDescription") }} />
              </div>
            </div>
            <div className="flex gap-3">
              <div className="shrink-0 mt-0.5">
                <svg className="w-5 h-5 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </div>
              <div>
                <span className="font-medium text-foreground">{t("jobs.fullRebuild")}</span>
                <p className="text-muted-foreground text-xs mt-0.5" dangerouslySetInnerHTML={{ __html: t("jobs.fullRebuildDescription") }} />
              </div>
            </div>
            <div className="flex gap-3">
              <div className="shrink-0 mt-0.5">
                <svg className="w-5 h-5 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                </svg>
              </div>
              <div>
                <span className="font-medium text-foreground">{t("jobs.generateThumbnails")}</span>
                <p className="text-muted-foreground text-xs mt-0.5" dangerouslySetInnerHTML={{ __html: t("jobs.generateThumbnailsDescription") }} />
              </div>
            </div>
            <div className="flex gap-3">
              <div className="shrink-0 mt-0.5">
                <svg className="w-5 h-5 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
              </div>
              <div>
                <span className="font-medium text-foreground">{t("jobs.regenerateThumbnails")}</span>
                <p className="text-muted-foreground text-xs mt-0.5" dangerouslySetInnerHTML={{ __html: t("jobs.regenerateThumbnailsDescription") }} />
              </div>
            </div>
            <div className="flex gap-3">
              <div className="shrink-0 mt-0.5">
                <svg className="w-5 h-5 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
              </div>
              <div>
                <span className="font-medium text-foreground">{t("jobs.batchMetadata")}</span>
                <p className="text-muted-foreground text-xs mt-0.5" dangerouslySetInnerHTML={{ __html: t("jobs.batchMetadataDescription") }} />
              </div>
            </div>
          </div>
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

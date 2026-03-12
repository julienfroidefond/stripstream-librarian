import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listJobs, fetchLibraries, rebuildIndex, rebuildThumbnails, regenerateThumbnails, IndexJobDto, LibraryDto } from "../../lib/api";
import { JobsList } from "../components/JobsList";
import { Card, CardHeader, CardTitle, CardContent, Button, FormField, FormSelect, FormRow } from "../components/ui";

export const dynamic = "force-dynamic";

export default async function JobsPage({ searchParams }: { searchParams: Promise<{ highlight?: string }> }) {
  const { highlight } = await searchParams;
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

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          Index Jobs
        </h1>
      </div>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Queue New Job</CardTitle>
        </CardHeader>
        <CardContent>
          <form>
            <FormRow>
              <FormField className="flex-1 max-w-xs">
                <FormSelect name="library_id" defaultValue="">
                  <option value="">All libraries</option>
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
                  Rebuild
                </Button>
                <Button type="submit" formAction={triggerFullRebuild} variant="warning">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                  Full Rebuild
                </Button>
                <Button type="submit" formAction={triggerThumbnailsRebuild} variant="secondary">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                  </svg>
                  Generate thumbnails
                </Button>
                <Button type="submit" formAction={triggerThumbnailsRegenerate} variant="warning">
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
                  Regenerate thumbnails
                </Button>
              </div>
            </FormRow>
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

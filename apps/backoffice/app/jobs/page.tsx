import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listJobs, fetchLibraries, rebuildIndex, IndexJobDto, LibraryDto } from "../../lib/api";
import { JobsList } from "../components/JobsList";
import { Card, CardHeader, Button, FormField, FormSelect, FormRow } from "../components/ui";

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

  return (
    <>
      <h1 className="text-3xl font-bold text-foreground mb-6 flex items-center gap-3">
        <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" /></svg>
        Index Jobs
      </h1>
      
      <Card className="mb-6">
        <form action={triggerRebuild}>
          <FormRow>
            <FormField>
              <FormSelect name="library_id" defaultValue="">
                <option value="">All libraries</option>
                {libraries.map((lib) => (
                  <option key={lib.id} value={lib.id}>
                    {lib.name}
                  </option>
                ))}
              </FormSelect>
            </FormField>
            <Button type="submit">🔄 Queue Rebuild</Button>
          </FormRow>
        </form>
        
        <form action={triggerFullRebuild} className="mt-3">
          <FormRow>
            <FormField>
              <FormSelect name="library_id" defaultValue="">
                <option value="">All libraries</option>
                {libraries.map((lib) => (
                  <option key={lib.id} value={lib.id}>
                    {lib.name}
                  </option>
                ))}
              </FormSelect>
            </FormField>
            <Button type="submit" variant="warning">🔁 Full Rebuild</Button>
          </FormRow>
        </form>
      </Card>
      
      <JobsList 
        initialJobs={jobs}
        libraries={libraryMap}
        highlightJobId={highlight}
      />
    </>
  );
}

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listJobs, fetchLibraries, rebuildIndex, IndexJobDto, LibraryDto } from "../../lib/api";
import { JobsList } from "../components/JobsList";

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

  const apiBaseUrl = process.env.API_BASE_URL || "http://api:8080";
  const apiToken = process.env.API_BOOTSTRAP_TOKEN || "";

  return (
    <>
      <h1>Index Jobs</h1>
      <div className="card">
        <form action={triggerRebuild}>
          <select name="library_id" defaultValue="">
            <option value="">All libraries</option>
            {libraries.map((lib) => (
              <option key={lib.id} value={lib.id}>
                {lib.name}
              </option>
            ))}
          </select>
          <button type="submit">Queue Rebuild</button>
        </form>
        <form action={triggerFullRebuild} style={{ marginTop: '12px' }}>
          <select name="library_id" defaultValue="">
            <option value="">All libraries</option>
            {libraries.map((lib) => (
              <option key={lib.id} value={lib.id}>
                {lib.name}
              </option>
            ))}
          </select>
          <button type="submit" className="full-rebuild-btn">Full Rebuild (Reindex All)</button>
        </form>
      </div>
      <JobsList 
        initialJobs={jobs}
        libraries={libraryMap}
        highlightJobId={highlight}
      />
    </>
  );
}

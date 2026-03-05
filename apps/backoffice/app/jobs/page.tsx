import { revalidatePath } from "next/cache";
import { listJobs, fetchLibraries, rebuildIndex, cancelJob, IndexJobDto, LibraryDto } from "../../lib/api";

export const dynamic = "force-dynamic";

export default async function JobsPage() {
  const [jobs, libraries] = await Promise.all([
    listJobs().catch(() => [] as IndexJobDto[]),
    fetchLibraries().catch(() => [] as LibraryDto[])
  ]);

  const libraryMap = new Map(libraries.map(l => [l.id, l.name]));

  async function triggerRebuild(formData: FormData) {
    "use server";
    const libraryId = formData.get("library_id") as string;
    await rebuildIndex(libraryId || undefined);
    revalidatePath("/jobs");
  }

  async function cancelJobAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await cancelJob(id);
    revalidatePath("/jobs");
  }

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
      </div>
      <table>
        <thead>
          <tr>
            <th>ID</th>
            <th>Library</th>
            <th>Type</th>
            <th>Status</th>
            <th>Created</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {jobs.map((job) => (
            <tr key={job.id}>
              <td>
                <code>{job.id.slice(0, 8)}</code>
              </td>
              <td>{job.library_id ? libraryMap.get(job.library_id) || job.library_id.slice(0, 8) : "—"}</td>
              <td>{job.type}</td>
              <td>
                <span className={`status-${job.status}`}>{job.status}</span>
                {job.error_opt && <span className="error-hint" title={job.error_opt}>!</span>}
              </td>
              <td>{new Date(job.created_at).toLocaleString()}</td>
              <td>
                {job.status === "pending" || job.status === "running" ? (
                  <form action={cancelJobAction}>
                    <input type="hidden" name="id" value={job.id} />
                    <button type="submit" className="cancel-btn">Cancel</button>
                  </form>
                ) : null}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

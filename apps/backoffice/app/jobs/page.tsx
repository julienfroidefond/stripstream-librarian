import { listJobs } from "../../lib/api";

export const dynamic = "force-dynamic";

export default async function JobsPage() {
  const jobs = await listJobs().catch(() => []);

  return (
    <>
      <h1>Index Jobs</h1>
      <div className="card">
        <form action="/jobs/rebuild" method="post">
          <input name="library_id" placeholder="optional library UUID" />
          <button type="submit">Queue Rebuild</button>
        </form>
      </div>
      <table>
        <thead>
          <tr>
            <th>ID</th>
            <th>Type</th>
            <th>Status</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          {jobs.map((job) => (
            <tr key={job.id}>
              <td>
                <code>{job.id}</code>
              </td>
              <td>{job.type}</td>
              <td>{job.status}</td>
              <td>{job.created_at}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

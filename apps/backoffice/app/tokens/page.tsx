import { listTokens } from "../../lib/api";

export const dynamic = "force-dynamic";

export default async function TokensPage({
  searchParams
}: {
  searchParams: Promise<{ created?: string }>;
}) {
  const params = await searchParams;
  const tokens = await listTokens().catch(() => []);

  return (
    <>
      <h1>API Tokens</h1>

      {params.created ? (
        <div className="card">
          <strong>Token created:</strong>
          <pre>{params.created}</pre>
        </div>
      ) : null}

      <div className="card">
        <form action="/tokens/create" method="post">
          <input name="name" placeholder="token name" required />
          <select name="scope" defaultValue="read">
            <option value="read">read</option>
            <option value="admin">admin</option>
          </select>
          <button type="submit">Create Token</button>
        </form>
      </div>

      <table>
        <thead>
          <tr>
            <th>Name</th>
            <th>Scope</th>
            <th>Prefix</th>
            <th>Revoked</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {tokens.map((token) => (
            <tr key={token.id}>
              <td>{token.name}</td>
              <td>{token.scope}</td>
              <td>
                <code>{token.prefix}</code>
              </td>
              <td>{token.revoked_at ? "yes" : "no"}</td>
              <td>
                <form className="inline" action="/tokens/revoke" method="post">
                  <input type="hidden" name="id" value={token.id} />
                  <button type="submit">Revoke</button>
                </form>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

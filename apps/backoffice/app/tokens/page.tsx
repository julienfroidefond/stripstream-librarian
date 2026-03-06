import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listTokens, createToken, revokeToken, TokenDto } from "../../lib/api";
import { Card, CardHeader, Button, FormField, FormInput, FormSelect, FormRow } from "../components/ui";

export const dynamic = "force-dynamic";

export default async function TokensPage({
  searchParams
}: {
  searchParams: Promise<{ created?: string }>;
}) {
  const params = await searchParams;
  const tokens = await listTokens().catch(() => [] as TokenDto[]);

  async function createTokenAction(formData: FormData) {
    "use server";
    const name = formData.get("name") as string;
    const scope = formData.get("scope") as string;
    if (name) {
      const result = await createToken(name, scope);
      revalidatePath("/tokens");
      redirect(`/tokens?created=${encodeURIComponent(result.token)}`);
    }
  }

  async function revokeTokenAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await revokeToken(id);
    revalidatePath("/tokens");
  }

  return (
    <>
      <h1 className="text-3xl font-bold text-foreground mb-6 flex items-center gap-3">
        <svg className="w-8 h-8 text-error" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" /></svg>
        API Tokens
      </h1>

      {params.created ? (
        <Card className="mb-6">
          <strong className="text-foreground block mb-2">Token created (copy it now, it won't be shown again):</strong>
          <pre className="p-4 bg-muted/10 rounded-lg text-sm font-mono text-foreground overflow-x-auto">{params.created}</pre>
        </Card>
      ) : null}

      <Card className="mb-6">
        <form action={createTokenAction}>
          <FormRow>
            <FormField>
              <FormInput name="name" placeholder="token name" required />
            </FormField>
            <FormField>
              <FormSelect name="scope" defaultValue="read">
                <option value="read">read</option>
                <option value="admin">admin</option>
              </FormSelect>
            </FormField>
            <Button type="submit">➕ Create Token</Button>
          </FormRow>
        </form>
      </Card>

      <Card className="overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-line bg-muted/5">
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Name</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Scope</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Prefix</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Revoked</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted uppercase tracking-wider">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {tokens.map((token) => (
                <tr key={token.id} className="hover:bg-muted/5">
                  <td className="px-4 py-3 text-sm text-foreground">{token.name}</td>
                  <td className="px-4 py-3 text-sm text-foreground">{token.scope}</td>
                  <td className="px-4 py-3 text-sm">
                    <code className="px-2 py-1 bg-muted/10 rounded font-mono text-foreground">{token.prefix}</code>
                  </td>
                  <td className="px-4 py-3 text-sm">
                    {token.revoked_at ? (
                      <span className="inline-flex px-2 py-1 rounded-full text-xs font-semibold bg-error-soft text-error">yes</span>
                    ) : (
                      <span className="inline-flex px-2 py-1 rounded-full text-xs font-semibold bg-success-soft text-success">no</span>
                    )}
                  </td>
                  <td className="px-4 py-3">
                    {!token.revoked_at && (
                      <form action={revokeTokenAction}>
                        <input type="hidden" name="id" value={token.id} />
                        <Button type="submit" variant="danger" size="sm">
                          🚫 Revoke
                        </Button>
                      </form>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </>
  );
}

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listTokens, createToken, revokeToken, deleteToken, TokenDto } from "../../lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, Badge, FormField, FormInput, FormSelect, FormRow } from "../components/ui";

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

  async function deleteTokenAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await deleteToken(id);
    revalidatePath("/tokens");
  }

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-destructive" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
          </svg>
          Jetons API
        </h1>
      </div>

      {params.created ? (
        <Card className="mb-6 border-success/50 bg-success/5">
          <CardHeader>
            <CardTitle className="text-success">Jeton créé</CardTitle>
            <CardDescription>Copiez-le maintenant, il ne sera plus affiché</CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="p-4 bg-background rounded-lg text-sm font-mono text-foreground overflow-x-auto border">{params.created}</pre>
          </CardContent>
        </Card>
      ) : null}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Créer un nouveau jeton</CardTitle>
          <CardDescription>Générer un nouveau jeton API avec la portée souhaitée</CardDescription>
        </CardHeader>
        <CardContent>
          <form action={createTokenAction}>
            <FormRow>
              <FormField className="flex-1 min-w-48">
                <FormInput name="name" placeholder="Nom du jeton" required />
              </FormField>
              <FormField className="w-32">
                <FormSelect name="scope" defaultValue="read">
                  <option value="read">Lecture</option>
                  <option value="admin">Admin</option>
                </FormSelect>
              </FormField>
              <Button type="submit">Créer le jeton</Button>
            </FormRow>
          </form>
        </CardContent>
      </Card>

      <Card className="overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-border/60 bg-muted/50">
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">Nom</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">Portée</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">Préfixe</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">Statut</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/60">
              {tokens.map((token) => (
                <tr key={token.id} className="hover:bg-accent/50 transition-colors">
                  <td className="px-4 py-3 text-sm text-foreground">{token.name}</td>
                  <td className="px-4 py-3 text-sm">
                    <Badge variant={token.scope === "admin" ? "destructive" : "secondary"}>
                      {token.scope}
                    </Badge>
                  </td>
                  <td className="px-4 py-3 text-sm">
                    <code className="px-2 py-1 bg-muted rounded font-mono text-foreground">{token.prefix}</code>
                  </td>
                  <td className="px-4 py-3 text-sm">
                    {token.revoked_at ? (
                      <Badge variant="error">Révoqué</Badge>
                    ) : (
                      <Badge variant="success">Actif</Badge>
                    )}
                  </td>
                  <td className="px-4 py-3">
                    {!token.revoked_at ? (
                      <form action={revokeTokenAction}>
                        <input type="hidden" name="id" value={token.id} />
                        <Button type="submit" variant="destructive" size="sm">
                          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
                          </svg>
                          Révoquer
                        </Button>
                      </form>
                    ) : (
                      <form action={deleteTokenAction}>
                        <input type="hidden" name="id" value={token.id} />
                        <Button type="submit" variant="destructive" size="sm">
                          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                          Supprimer
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

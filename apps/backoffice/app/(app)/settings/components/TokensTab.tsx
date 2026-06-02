import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { listTokens, createToken, revokeToken, deleteToken, updateToken, fetchUsers, createUser, deleteUser, updateUser, TokenDto, UserDto, fetchAllGenres, fetchUserGenreRestrictions, setUserGenreRestrictions } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, Badge, FormField, FormInput, FormSelect, FormRow } from "@/app/components/ui";
import { TokenUserSelect } from "@/app/components/TokenUserSelect";
import { UsernameEdit } from "@/app/components/UsernameEdit";
import { UserGenreRestrictions } from "@/app/components/UserGenreRestrictions";
import { getServerTranslations } from "@/lib/i18n/server";

export async function TokensTab({ createdToken }: { createdToken?: string }) {
  const { t } = await getServerTranslations();
  const tokens = await listTokens().catch(() => [] as TokenDto[]);
  const users = await fetchUsers().catch(() => [] as UserDto[]);
  const allGenres = await fetchAllGenres().catch(() => [] as string[]);
  const userRestrictions = await Promise.all(
    users.map((u) =>
      fetchUserGenreRestrictions(u.id)
        .then((r) => ({ userId: u.id, blocked: r.blocked_genres }))
        .catch(() => ({ userId: u.id, blocked: [] as string[] }))
    )
  );
  const restrictionsMap = Object.fromEntries(userRestrictions.map((r) => [r.userId, r.blocked]));

  async function createTokenAction(formData: FormData) {
    "use server";
    const name = formData.get("name") as string;
    const scope = formData.get("scope") as string;
    const userId = (formData.get("user_id") as string) || undefined;
    if (name) {
      const result = await createToken(name, scope, userId);
      revalidatePath("/settings");
      redirect(`/settings?tab=tokens&created=${encodeURIComponent(result.token)}`);
    }
  }

  async function revokeTokenAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await revokeToken(id);
    revalidatePath("/settings");
  }

  async function deleteTokenAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await deleteToken(id);
    revalidatePath("/settings");
  }

  async function createUserAction(formData: FormData) {
    "use server";
    const username = formData.get("username") as string;
    if (username) {
      await createUser(username);
      revalidatePath("/settings");
    }
  }

  async function deleteUserAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await deleteUser(id);
    revalidatePath("/settings");
  }

  async function renameUserAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    const username = formData.get("username") as string;
    if (username?.trim()) {
      await updateUser(id, username.trim());
      revalidatePath("/settings");
    }
  }

  async function setGenreRestrictionsAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    const raw = formData.get("blocked_genres") as string;
    const blockedGenres: string[] = raw ? JSON.parse(raw) : [];
    await setUserGenreRestrictions(id, blockedGenres);
    revalidatePath("/settings");
  }

  async function reassignTokenAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    const userId = (formData.get("user_id") as string) || null;
    await updateToken(id, userId);
    revalidatePath("/settings");
  }

  return (
    <>
      {/* ── Lecteurs ─────────────────────────────────────────── */}
      <div className="mb-2">
        <h2 className="text-xl font-semibold text-foreground">{t("users.title")}</h2>
      </div>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>{t("users.createNew")}</CardTitle>
          <CardDescription>{t("users.createDescription")}</CardDescription>
        </CardHeader>
        <CardContent>
          <form action={createUserAction}>
            <FormRow>
              <FormField className="flex-1 min-w-48">
                <FormInput name="username" placeholder={t("users.username")} required autoComplete="off" />
              </FormField>
              <Button type="submit">{t("users.createButton")}</Button>
            </FormRow>
          </form>
        </CardContent>
      </Card>

      <Card className="overflow-hidden mb-10">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-border/60 bg-muted/50">
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("users.name")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("users.tokenCount")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("status.read")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("status.reading")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("users.createdAt")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("users.blockedGenres")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("users.actions")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/60">
              <tr className="hover:bg-accent/50 transition-colors bg-destructive/5">
                <td className="px-4 py-3 text-sm font-medium text-foreground flex items-center gap-2">
                  {process.env.ADMIN_USERNAME ?? "admin"}
                  <Badge variant="destructive">{t("tokens.scopeAdmin")}</Badge>
                </td>
                <td className="px-4 py-3 text-sm text-muted-foreground">
                  {tokens.filter(tok => tok.scope === "admin" && !tok.revoked_at).length}
                </td>
                <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
              </tr>
              {(() => {
                const unassigned = tokens.filter(tok => tok.scope === "read" && !tok.user_id && !tok.revoked_at);
                if (unassigned.length === 0) return null;
                return (
                  <tr className="hover:bg-accent/50 transition-colors bg-warning/5">
                    <td className="px-4 py-3 text-sm font-medium text-muted-foreground italic">{t("tokens.noUser")}</td>
                    <td className="px-4 py-3 text-sm text-warning font-medium">{unassigned.length}</td>
                    <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                    <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                    <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                    <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                    <td className="px-4 py-3 text-sm text-muted-foreground/50">—</td>
                  </tr>
                );
              })()}
              {users.map((user) => (
                <tr key={user.id} className="hover:bg-accent/50 transition-colors">
                  <td className="px-4 py-3">
                    <UsernameEdit userId={user.id} currentUsername={user.username} action={renameUserAction} />
                  </td>
                  <td className="px-4 py-3 text-sm text-muted-foreground">{user.token_count}</td>
                  <td className="px-4 py-3 text-sm">
                    {user.books_read > 0
                      ? <span className="font-medium text-success">{user.books_read}</span>
                      : <span className="text-muted-foreground/50">—</span>}
                  </td>
                  <td className="px-4 py-3 text-sm">
                    {user.books_reading > 0
                      ? <span className="font-medium text-amber-500">{user.books_reading}</span>
                      : <span className="text-muted-foreground/50">—</span>}
                  </td>
                  <td className="px-4 py-3 text-sm text-muted-foreground">
                    {new Date(user.created_at).toLocaleDateString()}
                  </td>
                  <td className="px-4 py-3">
                    {(restrictionsMap[user.id] ?? []).length > 0 ? (
                      <div className="flex flex-wrap gap-1">
                        {(restrictionsMap[user.id] ?? []).map((g) => (
                          <span key={g} className="inline-block px-1.5 py-0.5 rounded text-xs font-medium bg-destructive/10 text-destructive border border-destructive/20">
                            {g}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <span className="text-muted-foreground/40 text-xs">—</span>
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <UserGenreRestrictions
                        userId={user.id}
                        initialBlockedGenres={restrictionsMap[user.id] ?? []}
                        allGenres={allGenres}
                        action={setGenreRestrictionsAction}
                      />
                      <form action={deleteUserAction}>
                        <input type="hidden" name="id" value={user.id} />
                        <Button type="submit" variant="destructive" size="xs">
                          <svg className="w-3.5 h-3.5 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                          {t("common.delete")}
                        </Button>
                      </form>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>

      {/* ── Tokens API ───────────────────────────────────────── */}
      <div className="mb-2">
        <h2 className="text-xl font-semibold text-foreground">{t("tokens.apiTokens")}</h2>
      </div>

      {createdToken && (
        <Card className="mb-6 border-success/50 bg-success/5">
          <CardHeader>
            <CardTitle className="text-success">{t("tokens.created")}</CardTitle>
            <CardDescription>{t("tokens.createdDescription")}</CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="p-4 bg-background rounded-lg text-sm font-mono text-foreground overflow-x-auto border">{createdToken}</pre>
          </CardContent>
        </Card>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>{t("tokens.createNew")}</CardTitle>
          <CardDescription>{t("tokens.createDescription")}</CardDescription>
        </CardHeader>
        <CardContent>
          <form action={createTokenAction}>
            <FormRow>
              <FormField className="flex-1 min-w-48">
                <FormInput name="name" placeholder={t("tokens.tokenName")} required autoComplete="off" />
              </FormField>
              <FormField className="w-32">
                <FormSelect name="scope" defaultValue="read">
                  <option value="read">{t("tokens.scopeRead")}</option>
                  <option value="admin">{t("tokens.scopeAdmin")}</option>
                </FormSelect>
              </FormField>
              <FormField className="w-48">
                <FormSelect name="user_id" defaultValue="">
                  <option value="">{t("tokens.noUser")}</option>
                  {users.map((user) => (
                    <option key={user.id} value={user.id}>{user.username}</option>
                  ))}
                </FormSelect>
              </FormField>
              <Button type="submit">{t("tokens.createButton")}</Button>
            </FormRow>
          </form>
        </CardContent>
      </Card>

      <Card className="overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-border/60 bg-muted/50">
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("tokens.name")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("tokens.user")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("tokens.scope")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("tokens.prefix")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("tokens.status")}</th>
                <th className="px-4 py-3 text-left text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("tokens.actions")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/60">
              {tokens.map((token) => (
                <tr key={token.id} className="hover:bg-accent/50 transition-colors">
                  <td className="px-4 py-3 text-sm text-foreground">{token.name}</td>
                  <td className="px-4 py-3 text-sm">
                    <TokenUserSelect
                      tokenId={token.id}
                      currentUserId={token.user_id}
                      users={users}
                      action={reassignTokenAction}
                      noUserLabel={t("tokens.noUser")}
                    />
                  </td>
                  <td className="px-4 py-3 text-sm">
                    <Badge variant={token.scope === "admin" ? "destructive" : "secondary"}>{token.scope}</Badge>
                  </td>
                  <td className="px-4 py-3 text-sm">
                    <code className="px-2 py-1 bg-muted rounded font-mono text-foreground">{token.prefix}</code>
                  </td>
                  <td className="px-4 py-3 text-sm">
                    {token.revoked_at
                      ? <Badge variant="error">{t("tokens.revoked")}</Badge>
                      : <Badge variant="success">{t("tokens.active")}</Badge>}
                  </td>
                  <td className="px-4 py-3">
                    {!token.revoked_at ? (
                      <form action={revokeTokenAction}>
                        <input type="hidden" name="id" value={token.id} />
                        <Button type="submit" variant="destructive" size="xs">
                          <svg className="w-3.5 h-3.5 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
                          </svg>
                          {t("tokens.revoke")}
                        </Button>
                      </form>
                    ) : (
                      <form action={deleteTokenAction}>
                        <input type="hidden" name="id" value={token.id} />
                        <Button type="submit" variant="destructive" size="xs">
                          <svg className="w-3.5 h-3.5 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                          {t("common.delete")}
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

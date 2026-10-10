"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, SettingsField, FormInput, Icon } from "@/app/components/ui";
import { UserDto, AnilistStatusDto, AnilistSyncReportDto, AnilistPullReportDto, AnilistSyncPreviewItemDto, AnilistSyncItemDto, AnilistPullItemDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import { useIsMounted } from "@/lib/useIsMounted";

export function AnilistTab({
  handleUpdateSetting,
  users,
  initialData,
}: {
  handleUpdateSetting: (key: string, value: unknown) => Promise<void>;
  users: UserDto[];
  initialData: Record<string, unknown> | null;
}) {
  const { t } = useTranslation();

  const mounted = useIsMounted();
  const origin = mounted ? window.location.origin : "";

  const [clientId, setClientId] = useState(initialData?.client_id ? String(initialData.client_id) : "");
  const [token, setToken] = useState(initialData?.access_token ? String(initialData.access_token) : "");
  const [userId, setUserId] = useState(initialData?.user_id ? String(initialData.user_id) : "");
  const [localUserId, setLocalUserId] = useState(initialData?.local_user_id ? String(initialData.local_user_id) : "");
  const [isTesting, setIsTesting] = useState(false);
  const [viewer, setViewer] = useState<AnilistStatusDto | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  const [isSyncing, setIsSyncing] = useState(false);
  const [syncReport, setSyncReport] = useState<AnilistSyncReportDto | null>(null);
  const [isPulling, setIsPulling] = useState(false);
  const [pullReport, setPullReport] = useState<AnilistPullReportDto | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [previewItems, setPreviewItems] = useState<AnilistSyncPreviewItemDto[] | null>(null);

  function buildAnilistSettings() {
    return {
      client_id: clientId || undefined,
      access_token: token || undefined,
      user_id: userId ? Number(userId) : undefined,
      local_user_id: localUserId || undefined,
    };
  }

  function handleConnect() {
    if (!clientId) return;
    // Save client_id first, then open OAuth URL
    handleUpdateSetting("anilist", buildAnilistSettings()).then(() => {
      window.location.href = `https://anilist.co/api/v2/oauth/authorize?client_id=${encodeURIComponent(clientId)}&response_type=token`;
    });
  }

  async function handleSaveToken() {
    await handleUpdateSetting("anilist", buildAnilistSettings());
  }

  async function handleTestConnection() {
    setIsTesting(true);
    setViewer(null);
    setTestError(null);
    try {
      // Save token first so the API reads the current value
      await handleUpdateSetting("anilist", buildAnilistSettings());
      const resp = await fetch("/api/anilist/status");
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Connection failed");
      setViewer(data);
      if (!userId && data.user_id) setUserId(String(data.user_id));
    } catch (e) {
      setTestError(e instanceof Error ? e.message : "Connection failed");
    } finally {
      setIsTesting(false);
    }
  }

  async function handlePreview() {
    setIsPreviewing(true);
    setPreviewItems(null);
    setActionError(null);
    try {
      const resp = await fetch("/api/anilist/sync/preview");
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Preview failed");
      setPreviewItems(data);
    } catch (e) {
      setActionError(e instanceof Error ? e.message : "Preview failed");
    } finally {
      setIsPreviewing(false);
    }
  }

  async function handleSync() {
    setIsSyncing(true);
    setSyncReport(null);
    setActionError(null);
    try {
      const resp = await fetch("/api/anilist/sync", { method: "POST" });
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Sync failed");
      setSyncReport(data);
    } catch (e) {
      setActionError(e instanceof Error ? e.message : "Sync failed");
    } finally {
      setIsSyncing(false);
    }
  }

  async function handlePull() {
    setIsPulling(true);
    setPullReport(null);
    setActionError(null);
    try {
      const resp = await fetch("/api/anilist/pull", { method: "POST" });
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Pull failed");
      setPullReport(data);
    } catch (e) {
      setActionError(e instanceof Error ? e.message : "Pull failed");
    } finally {
      setIsPulling(false);
    }
  }

  return (
    <>
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="link" size="md" />
            {t("settings.anilistTitle")}
          </CardTitle>
          <CardDescription>{t("settings.anilistDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">{t("settings.anilistConnectDesc")}</p>
          {/* Redirect URL info */}
          <div className="rounded-md bg-muted/50 border px-3 py-2 text-xs text-muted-foreground space-y-1">
            <p className="font-medium text-foreground">{t("settings.anilistRedirectUrlLabel")}</p>
            <code className="select-all font-mono">{origin ? `${origin}/anilist/callback` : "/anilist/callback"}</code>
            <p>{t("settings.anilistRedirectUrlHint")}</p>
          </div>
          <div className="flex gap-4">
            <SettingsField className="flex-1" label={t("settings.anilistClientId")}>
              <FormInput
                type="text"
                autoComplete="off"
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder={t("settings.anilistClientIdPlaceholder")}
              />
            </SettingsField>
          </div>
          <div className="flex items-center gap-3 flex-wrap">
            <Button onClick={handleConnect} disabled={!clientId}>
              <Icon name="link" size="sm" className="mr-2" />
              {t("settings.anilistConnectButton")}
            </Button>
            <Button onClick={handleTestConnection} disabled={isTesting || !token} variant="secondary">
              {isTesting ? (
                <><Icon name="spinner" size="sm" className="animate-spin mr-2" />{t("settings.testing")}</>
              ) : (
                <><Icon name="refresh" size="sm" className="mr-2" />{t("settings.anilistTestConnection")}</>
              )}
            </Button>
            {viewer && (
              <span className="text-sm text-success font-medium">
                {t("settings.anilistConnected")} <strong>{viewer.username}</strong>
                {" · "}
                <a href={viewer.site_url} target="_blank" rel="noopener noreferrer" className="underline">AniList</a>
              </span>
            )}
            {token && !viewer && (
              <span className="text-sm text-muted-foreground">{t("settings.anilistTokenPresent")}</span>
            )}
            {testError && <span className="text-sm text-destructive">{testError}</span>}
          </div>
          <details className="group">
            <summary className="text-sm text-muted-foreground cursor-pointer hover:text-foreground select-none">
              {t("settings.anilistManualToken")}
            </summary>
            <div className="mt-3 space-y-3">
              <div className="flex gap-4">
                <SettingsField className="flex-1" label={t("settings.anilistToken")}>
                  <FormInput
                    type="password"
                    autoComplete="off"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    placeholder={t("settings.anilistTokenPlaceholder")}
                  />
                </SettingsField>
                <SettingsField className="flex-1" label={t("settings.anilistUserId")}>
                  <FormInput
                    type="text"
                    autoComplete="off"
                    value={userId}
                    onChange={(e) => setUserId(e.target.value)}
                    placeholder={t("settings.anilistUserIdPlaceholder")}
                  />
                </SettingsField>
              </div>
              <Button onClick={handleSaveToken} disabled={!token}>
                {t("common.save")}
              </Button>
            </div>
          </details>
          <div className="border-t border-border/50 pt-4 mt-2">
            <p className="text-sm font-medium text-foreground mb-1">{t("settings.anilistLocalUserTitle")}</p>
            <p className="text-xs text-muted-foreground mb-3">{t("settings.anilistLocalUserDesc")}</p>
            <div className="flex items-center gap-3">
              <select
                value={localUserId}
                onChange={(e) => {
                  const newLocalUserId = e.target.value;
                  setLocalUserId(newLocalUserId);
                  handleUpdateSetting("anilist", {
                    ...buildAnilistSettings(),
                    local_user_id: newLocalUserId || undefined,
                  });
                }}
                autoComplete="off"
                className="flex-1 text-sm border border-border rounded-lg px-3 py-2.5 bg-background focus:outline-none focus:ring-2 focus:ring-ring h-10"
              >
                <option value="">{t("settings.anilistLocalUserNone")}</option>
                {users.map((u) => (
                  <option key={u.id} value={u.id}>{u.username}</option>
                ))}
              </select>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icon name="refresh" size="md" />
            {t("settings.anilistSyncTitle")}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-6">
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">{t("settings.anilistSyncDesc")}</p>
              <div className="flex items-center gap-2 flex-wrap">
                <Button onClick={handlePreview} disabled={isPreviewing} variant="secondary">
                  {isPreviewing ? (
                    <><Icon name="spinner" size="sm" className="animate-spin mr-2" />{t("settings.anilistPreviewing")}</>
                  ) : (
                    <><Icon name="eye" size="sm" className="mr-2" />{t("settings.anilistPreviewButton")}</>
                  )}
                </Button>
                <Button onClick={handleSync} disabled={isSyncing}>
                  {isSyncing ? (
                    <><Icon name="spinner" size="sm" className="animate-spin mr-2" />{t("settings.anilistSyncing")}</>
                  ) : (
                    <><Icon name="refresh" size="sm" className="mr-2" />{t("settings.anilistSyncButton")}</>
                  )}
                </Button>
              </div>
              {syncReport && (
                <div className="mt-2 border rounded-lg overflow-hidden">
                  <div className="px-4 py-2 bg-muted/50 flex items-center gap-3">
                    <span className="text-sm text-success font-medium">{t("settings.anilistSynced", { count: String(syncReport.synced) })}</span>
                    {syncReport.skipped > 0 && <span className="text-sm text-muted-foreground">{t("settings.anilistSkipped", { count: String(syncReport.skipped) })}</span>}
                    {syncReport.errors.length > 0 && <span className="text-sm text-destructive">{t("settings.anilistErrors", { count: String(syncReport.errors.length) })}</span>}
                  </div>
                  {syncReport.items.length > 0 && (
                    <div className="divide-y max-h-60 overflow-y-auto">
                      {syncReport.items.map((item: AnilistSyncItemDto) => (
                        <div key={item.series_name} className="flex items-center justify-between px-4 py-2 text-sm">
                          <a
                            href={item.anilist_url ?? `https://anilist.co/manga/`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="truncate font-medium hover:underline min-w-0 mr-3"
                          >
                            {item.anilist_title ?? item.series_name}
                          </a>
                          <div className="flex items-center gap-2 shrink-0">
                            <span className={`text-xs px-1.5 py-0.5 rounded-full font-medium ${
                              item.status === "COMPLETED" ? "bg-green-500/15 text-green-600" :
                              item.status === "CURRENT" ? "bg-blue-500/15 text-blue-600" :
                              "bg-muted text-muted-foreground"
                            }`}>{item.status}</span>
                            {item.progress_volumes > 0 && (
                              <span className="text-xs text-muted-foreground">{item.progress_volumes} vol.</span>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                  {syncReport.errors.map((err: string, i: number) => (
                    <p key={i} className="text-xs text-destructive px-4 py-1 border-t">{err}</p>
                  ))}
                </div>
              )}
            </div>
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">{t("settings.anilistPullDesc")}</p>
              <Button onClick={handlePull} disabled={isPulling}>
                {isPulling ? (
                  <><Icon name="spinner" size="sm" className="animate-spin mr-2" />{t("settings.anilistPulling")}</>
                ) : (
                  <><Icon name="refresh" size="sm" className="mr-2" />{t("settings.anilistPullButton")}</>
                )}
              </Button>
              {pullReport && (
                <div className="mt-2 border rounded-lg overflow-hidden">
                  <div className="px-4 py-2 bg-muted/50 flex items-center gap-3">
                    <span className="text-sm text-success font-medium">{t("settings.anilistUpdated", { count: String(pullReport.updated) })}</span>
                    {pullReport.skipped > 0 && <span className="text-sm text-muted-foreground">{t("settings.anilistSkipped", { count: String(pullReport.skipped) })}</span>}
                    {pullReport.errors.length > 0 && <span className="text-sm text-destructive">{t("settings.anilistErrors", { count: String(pullReport.errors.length) })}</span>}
                  </div>
                  {pullReport.items.length > 0 && (
                    <div className="divide-y max-h-60 overflow-y-auto">
                      {pullReport.items.map((item: AnilistPullItemDto) => (
                        <div key={item.series_name} className="flex items-center justify-between px-4 py-2 text-sm">
                          <a
                            href={item.anilist_url ?? `https://anilist.co/manga/`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="truncate font-medium hover:underline min-w-0 mr-3"
                          >
                            {item.anilist_title ?? item.series_name}
                          </a>
                          <div className="flex items-center gap-2 shrink-0">
                            <span className={`text-xs px-1.5 py-0.5 rounded-full font-medium ${
                              item.anilist_status === "COMPLETED" ? "bg-green-500/15 text-green-600" :
                              item.anilist_status === "CURRENT" ? "bg-blue-500/15 text-blue-600" :
                              item.anilist_status === "PLANNING" ? "bg-amber-500/15 text-amber-600" :
                              "bg-muted text-muted-foreground"
                            }`}>{item.anilist_status}</span>
                            <span className="text-xs text-muted-foreground">{item.books_updated} {t("dashboard.books").toLowerCase()}</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                  {pullReport.errors.map((err: string, i: number) => (
                    <p key={i} className="text-xs text-destructive px-4 py-1 border-t">{err}</p>
                  ))}
                </div>
              )}
            </div>
          </div>
          {actionError && <p className="text-sm text-destructive">{actionError}</p>}
          {previewItems !== null && (
            <div className="mt-2 border rounded-lg overflow-hidden">
              <div className="px-4 py-2 bg-muted/50 flex items-center justify-between">
                <span className="text-sm font-medium">{t("settings.anilistPreviewTitle", { count: String(previewItems.length) })}</span>
                <button onClick={() => setPreviewItems(null)} className="text-xs text-muted-foreground hover:text-foreground">✕</button>
              </div>
              {previewItems.length === 0 ? (
                <p className="text-sm text-muted-foreground px-4 py-3">{t("settings.anilistPreviewEmpty")}</p>
              ) : (
                <div className="divide-y">
                  {previewItems.map((item) => (
                    <div key={`${item.anilist_id}-${item.series_name}`} className="flex items-center justify-between px-4 py-2 text-sm">
                      <div className="flex items-center gap-2 min-w-0">
                        <a
                          href={item.anilist_url ?? `https://anilist.co/manga/${item.anilist_id}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="truncate font-medium hover:underline"
                        >
                          {item.anilist_title ?? item.series_name}
                        </a>
                        {item.anilist_title && item.anilist_title !== item.series_name && (
                          <span className="text-muted-foreground truncate hidden sm:inline">— {item.series_name}</span>
                        )}
                      </div>
                      <div className="flex items-center gap-3 shrink-0 ml-3">
                        <span className="text-xs text-muted-foreground">{item.books_read}/{item.book_count}</span>
                        <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                          item.status === "COMPLETED" ? "bg-success/15 text-success" :
                          item.status === "CURRENT" ? "bg-blue-500/15 text-blue-600" :
                          "bg-muted text-muted-foreground"
                        }`}>
                          {item.status}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </>
  );
}

"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, Button, FormInput, Icon } from "@/app/components/ui";
import { UserDto, AnilistStatusDto, AnilistSyncReportDto, AnilistPullReportDto, AnilistSyncPreviewItemDto, AnilistSyncItemDto, AnilistPullItemDto } from "@/lib/api";
import { browserApi, browserErrorMessage } from "@/lib/api/browser";
import { useTranslation } from "@/lib/i18n/context";
import { useIsMounted } from "@/lib/useIsMounted";
import type { AnilistSettings } from "@/lib/settingsSections";
import { useSettingsDraft, useSettingsSection } from "../SettingsDraftProvider";

export function AnilistTab({ users }: { users: UserDto[] }) {
  const { t } = useTranslation();

  const mounted = useIsMounted();
  const origin = mounted ? window.location.origin : "";
  const draft = useSettingsDraft();
  const section = useSettingsSection<AnilistSettings>("anilist");
  const value = section.value;

  // Raw inputs kept local until an explicit action saves them: the card has a
  // "Connect" flow and a manual-token form, neither of which saves per keystroke.
  const [clientId, setClientId] = useState(value.client_id);
  const [token, setToken] = useState(value.access_token);
  const [userId, setUserId] = useState(value.user_id != null ? String(value.user_id) : "");
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

  function buildAnilistSettings(localUserId?: string): AnilistSettings {
    return {
      client_id: clientId,
      access_token: token,
      user_id: userId ? Number(userId) : null,
      local_user_id: localUserId ?? value.local_user_id,
    };
  }

  async function save(settings: AnilistSettings) {
    section.set(settings);
    const result = await section.save();
    // A save may normalize the section, and a validation error or rollback
    // restores it: mirror the acknowledged value back into the local inputs.
    if (result.status !== "unchanged") {
      const stored = draft.get<AnilistSettings>("anilist");
      if (stored) {
        setClientId(stored.client_id);
        setToken(stored.access_token);
        setUserId(stored.user_id != null ? String(stored.user_id) : "");
      }
    }
    return result;
  }

  async function handleConnect() {
    if (!clientId) return;
    // Save client_id first, then open OAuth URL
    const result = await save(buildAnilistSettings());
    if (result.status === "error") return;
    window.location.href = `https://anilist.co/api/v2/oauth/authorize?client_id=${encodeURIComponent(clientId)}&response_type=token`;
  }

  async function handleSaveToken() {
    await save(buildAnilistSettings());
  }

  async function handleTestConnection() {
    setIsTesting(true);
    setViewer(null);
    setTestError(null);
    try {
      // Save token first so the API reads the current value
      await save(buildAnilistSettings());
      const data = await browserApi.anilist.status();
      setViewer(data);
      if (!userId && data.user_id) setUserId(String(data.user_id));
    } catch (e) {
      setTestError(browserErrorMessage(e, "Connection failed"));
    } finally {
      setIsTesting(false);
    }
  }

  async function handlePreview() {
    setIsPreviewing(true);
    setPreviewItems(null);
    setActionError(null);
    try {
      const data = await browserApi.anilist.preview();
      setPreviewItems(data);
    } catch (e) {
      setActionError(browserErrorMessage(e, "Preview failed"));
    } finally {
      setIsPreviewing(false);
    }
  }

  async function handleSync() {
    setIsSyncing(true);
    setSyncReport(null);
    setActionError(null);
    try {
      const data = await browserApi.anilist.sync();
      setSyncReport(data);
    } catch (e) {
      setActionError(browserErrorMessage(e, "Sync failed"));
    } finally {
      setIsSyncing(false);
    }
  }

  async function handlePull() {
    setIsPulling(true);
    setPullReport(null);
    setActionError(null);
    try {
      const data = await browserApi.anilist.pull();
      setPullReport(data);
    } catch (e) {
      setActionError(browserErrorMessage(e, "Pull failed"));
    } finally {
      setIsPulling(false);
    }
  }

  return (
    <>
      <SettingsCard icon="link" title={t("settings.anilistTitle")} description={t("settings.anilistDesc")}>
        <p className="text-sm text-muted-foreground">{t("settings.anilistConnectDesc")}</p>
        {/* Redirect URL info */}
        <div className="settings-well space-y-1 rounded-xl px-3.5 py-2.5 text-xs text-muted-foreground">
          <p className="font-medium text-foreground">{t("settings.anilistRedirectUrlLabel")}</p>
          <code className="select-all font-mono">{origin ? `${origin}/anilist/callback` : "/anilist/callback"}</code>
          <p>{t("settings.anilistRedirectUrlHint")}</p>
        </div>
        <SettingsRow label={t("settings.anilistClientId")} htmlFor="anilist-client-id">
          <FormInput
            id="anilist-client-id"
            type="text"
            autoComplete="off"
            value={clientId}
            onChange={(e) => setClientId(e.target.value)}
            placeholder={t("settings.anilistClientIdPlaceholder")}
          />
        </SettingsRow>
        <div className="flex flex-wrap items-center gap-3">
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
            <span className="text-sm font-medium text-success">
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
          <summary className="cursor-pointer select-none text-sm text-muted-foreground hover:text-foreground">
            {t("settings.anilistManualToken")}
          </summary>
          <div className="mt-3 space-y-3">
            <SettingsRow label={t("settings.anilistToken")} htmlFor="anilist-token">
              <FormInput
                id="anilist-token"
                type="password"
                autoComplete="off"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder={t("settings.anilistTokenPlaceholder")}
              />
            </SettingsRow>
            <SettingsRow label={t("settings.anilistUserId")} htmlFor="anilist-user-id">
              <FormInput
                id="anilist-user-id"
                type="text"
                autoComplete="off"
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                placeholder={t("settings.anilistUserIdPlaceholder")}
              />
            </SettingsRow>
            <Button onClick={handleSaveToken} disabled={!token}>
              {t("common.save")}
            </Button>
          </div>
        </details>
        <SettingsRow
          label={t("settings.anilistLocalUserTitle")}
          description={t("settings.anilistLocalUserDesc")}
          controlClassName="sm:max-w-none"
        >
          <select
            value={value.local_user_id}
            onChange={(e) => {
              const next = e.target.value;
              void save(buildAnilistSettings(next));
            }}
            autoComplete="off"
            className="h-10 w-full px-3 py-2.5 text-sm sm:max-w-md"
          >
            <option value="">{t("settings.anilistLocalUserNone")}</option>
            {users.map((u) => (
              <option key={u.id} value={u.id}>{u.username}</option>
            ))}
          </select>
        </SettingsRow>
      </SettingsCard>

      <SettingsCard icon="refresh" title={t("settings.anilistSyncTitle")}>
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
              <div className="settings-well mt-2 overflow-hidden rounded-xl">
                <div className="flex items-center gap-3 px-4 py-2.5">
                  <span className="text-sm text-success font-medium">{t("settings.anilistSynced", { count: String(syncReport.synced) })}</span>
                  {syncReport.skipped > 0 && <span className="text-sm text-muted-foreground">{t("settings.anilistSkipped", { count: String(syncReport.skipped) })}</span>}
                  {syncReport.errors.length > 0 && <span className="text-sm text-destructive">{t("settings.anilistErrors", { count: String(syncReport.errors.length) })}</span>}
                </div>
                {syncReport.items.length > 0 && (
                  <div className="max-h-60 divide-y divide-border/40 overflow-y-auto">
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
                            item.status === "COMPLETED" ? "bg-success/15 text-success" :
                            item.status === "CURRENT" ? "bg-info/15 text-info" :
                            "settings-well text-muted-foreground"
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
              <div className="settings-well mt-2 overflow-hidden rounded-xl">
                <div className="flex items-center gap-3 px-4 py-2.5">
                  <span className="text-sm text-success font-medium">{t("settings.anilistUpdated", { count: String(pullReport.updated) })}</span>
                  {pullReport.skipped > 0 && <span className="text-sm text-muted-foreground">{t("settings.anilistSkipped", { count: String(pullReport.skipped) })}</span>}
                  {pullReport.errors.length > 0 && <span className="text-sm text-destructive">{t("settings.anilistErrors", { count: String(pullReport.errors.length) })}</span>}
                </div>
                {pullReport.items.length > 0 && (
                  <div className="max-h-60 divide-y divide-border/40 overflow-y-auto">
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
                            item.anilist_status === "COMPLETED" ? "bg-success/15 text-success" :
                            item.anilist_status === "CURRENT" ? "bg-info/15 text-info" :
                            item.anilist_status === "PLANNING" ? "bg-warning/15 text-warning" :
                            "settings-well text-muted-foreground"
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
          <div className="settings-well mt-2 overflow-hidden rounded-xl">
            <div className="flex items-center justify-between px-4 py-2.5">
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
                      <span className={`text-3xs px-1.5 py-0.5 rounded-full font-medium ${
                        item.status === "COMPLETED" ? "bg-success/15 text-success" :
                        item.status === "CURRENT" ? "bg-info/15 text-info" :
                        "settings-well text-muted-foreground"
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
      </SettingsCard>
    </>
  );
}

"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, Icon } from "@/app/components/ui";
import { KomgaSyncResponse, KomgaSyncReportSummary, UserDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

export function KomgaSyncCard({ users }: { users: UserDto[] }) {
  const { t, locale } = useTranslation();
  const [komgaUrl, setKomgaUrl] = useState("");
  const [komgaUsername, setKomgaUsername] = useState("");
  const [komgaPassword, setKomgaPassword] = useState("");
  const [komgaUserId, setKomgaUserId] = useState(users[0]?.id ?? "");
  const [isSyncing, setIsSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<KomgaSyncResponse | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [showUnmatched, setShowUnmatched] = useState(false);
  const [showMatchedBooks, setShowMatchedBooks] = useState(false);
  const [reports, setReports] = useState<KomgaSyncReportSummary[]>([]);
  const [selectedReport, setSelectedReport] = useState<KomgaSyncResponse | null>(null);
  const [showReportUnmatched, setShowReportUnmatched] = useState(false);
  const [showReportMatchedBooks, setShowReportMatchedBooks] = useState(false);

  const syncNewlyMarkedSet = useMemo(
    () => new Set(syncResult?.newly_marked_books ?? []),
    [syncResult?.newly_marked_books],
  );
  const reportNewlyMarkedSet = useMemo(
    () => new Set(selectedReport?.newly_marked_books ?? []),
    [selectedReport?.newly_marked_books],
  );

  const fetchReports = useCallback(async () => {
    try {
      const resp = await fetch("/api/komga/reports");
      if (resp.ok) setReports(await resp.json());
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    fetchReports();
    fetch("/api/settings/komga").then(r => r.ok ? r.json() : null).then(data => {
      if (data) {
        if (data.url) setKomgaUrl(data.url);
        if (data.username) setKomgaUsername(data.username);
        if (data.user_id) setKomgaUserId(data.user_id);
      }
    }).catch(() => {});
  }, [fetchReports]);

  async function handleViewReport(id: string) {
    setSelectedReport(null);
    setShowReportUnmatched(false);
    setShowReportMatchedBooks(false);
    try {
      const resp = await fetch(`/api/komga/reports/${id}`);
      if (resp.ok) setSelectedReport(await resp.json());
    } catch { /* ignore */ }
  }

  async function handleKomgaSync() {
    setIsSyncing(true);
    setSyncResult(null);
    setSyncError(null);
    setShowUnmatched(false);
    setShowMatchedBooks(false);
    try {
      const response = await fetch("/api/komga/sync", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: komgaUrl, username: komgaUsername, password: komgaPassword, user_id: komgaUserId }),
      });
      const data = await response.json();
      if (!response.ok) {
        setSyncError(data.error || "Sync failed");
      } else {
        setSyncResult(data);
        fetchReports();
        fetch("/api/settings/komga", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ value: { url: komgaUrl, username: komgaUsername, user_id: komgaUserId } }),
        }).catch(() => {});
      }
    } catch {
      setSyncError("Failed to connect to sync endpoint");
    } finally {
      setIsSyncing(false);
    }
  }

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="refresh" size="md" />
          {t("settings.komgaSync")}
        </CardTitle>
        <CardDescription>{t("settings.komgaDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.komgaUrl")}</label>
              <FormInput type="url" placeholder="https://komga.example.com" value={komgaUrl} onChange={(e) => setKomgaUrl(e.target.value)} />
            </FormField>
          </div>
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.username")}</label>
              <FormInput value={komgaUsername} onChange={(e) => setKomgaUsername(e.target.value)} />
            </FormField>
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.password")}</label>
              <FormInput type="password" autoComplete="off" value={komgaPassword} onChange={(e) => setKomgaPassword(e.target.value)} />
            </FormField>
          </div>
          {users.length > 0 && (
            <div className="flex gap-4">
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("users.title")}</label>
                <FormSelect value={komgaUserId} onChange={(e) => setKomgaUserId(e.target.value)}>
                  {users.map((u) => (
                    <option key={u.id} value={u.id}>{u.username}</option>
                  ))}
                </FormSelect>
              </FormField>
            </div>
          )}
          <Button onClick={handleKomgaSync} disabled={isSyncing || !komgaUrl || !komgaUsername || !komgaPassword || !komgaUserId}>
            {isSyncing ? (
              <><Icon name="spinner" size="sm" className="animate-spin -ml-1 mr-2" />{t("settings.syncing")}</>
            ) : (
              <><Icon name="refresh" size="sm" className="mr-2" />{t("settings.syncReadBooks")}</>
            )}
          </Button>
          {syncError && <div className="p-3 rounded-lg bg-destructive/10 text-destructive">{syncError}</div>}
          {syncResult && (
            <div className="space-y-3">
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 p-4 bg-muted/30 rounded-lg">
                <div>
                  <p className="text-sm text-muted-foreground">{t("settings.komgaRead")}</p>
                  <p className="text-2xl font-semibold">{syncResult.total_komga_read}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">{t("settings.matched")}</p>
                  <p className="text-2xl font-semibold">{syncResult.matched}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">{t("settings.alreadyRead")}</p>
                  <p className="text-2xl font-semibold">{syncResult.already_read}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">{t("settings.newlyMarked")}</p>
                  <p className="text-2xl font-semibold text-success">{syncResult.newly_marked}</p>
                </div>
              </div>
              {syncResult.matched_books.length > 0 && (
                <div>
                  <button type="button" onClick={() => setShowMatchedBooks(!showMatchedBooks)} className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1">
                    <Icon name={showMatchedBooks ? "chevronDown" : "chevronRight"} size="sm" />
                    {t("settings.matchedBooks", { count: syncResult.matched_books.length, plural: syncResult.matched_books.length !== 1 ? "s" : "" })}
                  </button>
                  {showMatchedBooks && (
                    <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-success/5 rounded-lg text-sm space-y-1">
                      {syncResult.matched_books.map((title, i) => (
                        <p key={i} className="text-foreground truncate flex items-center gap-1.5" title={title}>
                          {syncNewlyMarkedSet.has(title) && <Icon name="check" size="sm" className="text-success shrink-0" />}
                          {title}
                        </p>
                      ))}
                    </div>
                  )}
                </div>
              )}
              {syncResult.unmatched.length > 0 && (
                <div>
                  <button type="button" onClick={() => setShowUnmatched(!showUnmatched)} className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1">
                    <Icon name={showUnmatched ? "chevronDown" : "chevronRight"} size="sm" />
                    {t("settings.unmatchedBooks", { count: syncResult.unmatched.length, plural: syncResult.unmatched.length !== 1 ? "s" : "" })}
                  </button>
                  {showUnmatched && (
                    <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                      {syncResult.unmatched.map((title, i) => (
                        <p key={i} className="text-muted-foreground truncate" title={title}>{title}</p>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
          {reports.length > 0 && (
            <div className="border-t border-border pt-4">
              <h3 className="text-sm font-medium text-foreground mb-3">{t("settings.syncHistory")}</h3>
              <div className="space-y-2">
                {reports.map((r) => (
                  <button
                    key={r.id}
                    type="button"
                    onClick={() => handleViewReport(r.id)}
                    className={`w-full text-left p-3 rounded-lg border transition-colors ${
                      selectedReport?.id === r.id ? "border-primary bg-primary/5" : "border-border/60 bg-muted/20 hover:bg-muted/40"
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-sm font-medium text-foreground">{new Date(r.created_at).toLocaleString(locale)}</span>
                      <span className="text-xs text-muted-foreground truncate ml-2" title={r.komga_url}>{r.komga_url}</span>
                    </div>
                    <div className="flex gap-4 mt-1 text-xs text-muted-foreground">
                      <span>{r.total_komga_read} {t("settings.read")}</span>
                      <span>{r.matched} {t("settings.matched").toLowerCase()}</span>
                      <span className="text-success">{r.newly_marked} {t("settings.new")}</span>
                      {r.unmatched_count > 0 && <span className="text-warning">{r.unmatched_count} {t("settings.unmatched")}</span>}
                    </div>
                  </button>
                ))}
              </div>
              {selectedReport && (
                <div className="mt-3 space-y-3">
                  <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 p-4 bg-muted/30 rounded-lg">
                    <div>
                      <p className="text-sm text-muted-foreground">{t("settings.komgaRead")}</p>
                      <p className="text-2xl font-semibold">{selectedReport.total_komga_read}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">{t("settings.matched")}</p>
                      <p className="text-2xl font-semibold">{selectedReport.matched}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">{t("settings.alreadyRead")}</p>
                      <p className="text-2xl font-semibold">{selectedReport.already_read}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">{t("settings.newlyMarked")}</p>
                      <p className="text-2xl font-semibold text-success">{selectedReport.newly_marked}</p>
                    </div>
                  </div>
                  {selectedReport.matched_books && selectedReport.matched_books.length > 0 && (
                    <div>
                      <button type="button" onClick={() => setShowReportMatchedBooks(!showReportMatchedBooks)} className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1">
                        <Icon name={showReportMatchedBooks ? "chevronDown" : "chevronRight"} size="sm" />
                        {t("settings.matchedBooks", { count: selectedReport.matched_books.length, plural: selectedReport.matched_books.length !== 1 ? "s" : "" })}
                      </button>
                      {showReportMatchedBooks && (
                        <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-success/5 rounded-lg text-sm space-y-1">
                          {selectedReport.matched_books.map((title, i) => (
                            <p key={i} className="text-foreground truncate flex items-center gap-1.5" title={title}>
                              {reportNewlyMarkedSet.has(title) && <Icon name="check" size="sm" className="text-success shrink-0" />}
                              {title}
                            </p>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                  {selectedReport.unmatched.length > 0 && (
                    <div>
                      <button type="button" onClick={() => setShowReportUnmatched(!showReportUnmatched)} className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1">
                        <Icon name={showReportUnmatched ? "chevronDown" : "chevronRight"} size="sm" />
                        {t("settings.unmatchedBooks", { count: selectedReport.unmatched.length, plural: selectedReport.unmatched.length !== 1 ? "s" : "" })}
                      </button>
                      {showReportUnmatched && (
                        <div className="mt-2 max-h-60 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                          {selectedReport.unmatched.map((title, i) => (
                            <p key={i} className="text-muted-foreground truncate" title={title}>{title}</p>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

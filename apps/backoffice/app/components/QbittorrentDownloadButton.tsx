"use client";

import { useState, useEffect, createContext, useContext, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon, Button } from "./ui";
import { useTranslation } from "@/lib/i18n/context";

interface QbContextValue {
  configured: boolean;
  onDownloadStarted?: () => void;
}

const QbConfigContext = createContext<QbContextValue>({ configured: false });

export function QbittorrentProvider({ children, initialConfigured, onDownloadStarted }: { children: ReactNode; initialConfigured?: boolean; onDownloadStarted?: () => void }) {
  const [configured, setConfigured] = useState(initialConfigured ?? false);

  useEffect(() => {
    // Skip client fetch if server already told us
    if (initialConfigured !== undefined) return;
    fetch("/api/settings/qbittorrent")
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        setConfigured(!!(data && data.url && data.url.trim() && data.username && data.username.trim()));
      })
      .catch(() => setConfigured(false));
  }, [initialConfigured]);

  return <QbConfigContext.Provider value={{ configured, onDownloadStarted }}>{children}</QbConfigContext.Provider>;
}

export function QbittorrentDownloadButton({
  downloadUrl,
  releaseId,
  libraryId,
  seriesName,
  expectedVolumes,
  allVolumes,
  alwaysShowReplace,
}: {
  downloadUrl: string;
  releaseId: string;
  libraryId?: string;
  seriesName?: string;
  expectedVolumes?: number[];
  allVolumes?: number[];
  /** Show replace button even when allVolumes == expectedVolumes (e.g. in Prowlarr search modal) */
  alwaysShowReplace?: boolean;
}) {
  const { t } = useTranslation();
  const { configured, onDownloadStarted } = useContext(QbConfigContext);
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);

  if (!configured) return null;

  const showReplaceButton = allVolumes && allVolumes.length > 0
    && (alwaysShowReplace || (expectedVolumes && allVolumes.length > expectedVolumes.length));

  async function handleSend(volumes?: number[], replaceExisting = false) {
    setSending(true);
    setError(null);
    try {
      const resp = await fetch("/api/qbittorrent/add", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url: downloadUrl,
          ...(libraryId && { library_id: libraryId }),
          ...(seriesName && { series_name: seriesName }),
          ...((volumes || expectedVolumes) && { expected_volumes: volumes || expectedVolumes }),
          ...(replaceExisting && { replace_existing: true }),
        }),
      });
      const data = await resp.json();
      if (data.error) {
        setError(data.error);
      } else if (data.success) {
        setSent(true);
        onDownloadStarted?.();
        setTimeout(() => setSent(false), 5000);
      } else {
        setError(data.message || t("prowlarr.sentError"));
      }
    } catch {
      setError(t("prowlarr.sentError"));
    } finally {
      setSending(false);
    }
  }

  return (
    <>
      <div className="inline-flex items-center gap-0.5">
        <button
          type="button"
          onClick={() => handleSend()}
          disabled={sending}
          className={`inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-50 shrink-0 ${
            sent
              ? "text-green-500"
              : error
                ? "text-destructive"
                : "text-primary hover:bg-primary/10"
          }`}
          title={sent ? t("prowlarr.sentSuccess") : error || t("prowlarr.sendToQbittorrent")}
        >
          {sending ? (
            <Icon name="spinner" size="sm" className="animate-spin" />
          ) : sent ? (
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M3 8l4 4 6-7" />
            </svg>
          ) : (
            <Icon name="download" size="sm" />
          )}
        </button>

        {showReplaceButton && (
          <button
            type="button"
            onClick={() => setShowConfirm(true)}
            disabled={sending}
            className="inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-50 shrink-0 text-warning hover:bg-warning/10"
            title={t("prowlarr.replaceAndDownload")}
          >
            <Icon name="refresh" size="sm" />
          </button>
        )}
      </div>

      {showConfirm && createPortal(
        <>
          <div className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50" onClick={() => setShowConfirm(false)} />
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-sm overflow-hidden animate-in fade-in zoom-in-95 duration-200">
              <div className="p-6">
                <h3 className="text-lg font-semibold text-foreground mb-2">
                  {t("prowlarr.replaceAndDownload")}
                </h3>
                <p className="text-sm text-muted-foreground">
                  {t("prowlarr.confirmReplace")}
                </p>
              </div>
              <div className="flex justify-end gap-2 px-6 pb-6">
                <Button variant="outline" size="sm" onClick={() => setShowConfirm(false)}>
                  {t("common.cancel")}
                </Button>
                <Button variant="destructive" size="sm" onClick={() => { setShowConfirm(false); handleSend(allVolumes, true); }}>
                  {t("prowlarr.replaceAndDownload")}
                </Button>
              </div>
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  );
}

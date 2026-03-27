"use client";

import { useState, useEffect, createContext, useContext, type ReactNode } from "react";
import { Icon } from "./ui";
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
}: {
  downloadUrl: string;
  releaseId: string;
  libraryId?: string;
  seriesName?: string;
  expectedVolumes?: number[];
}) {
  const { t } = useTranslation();
  const { configured, onDownloadStarted } = useContext(QbConfigContext);
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!configured) return null;

  async function handleSend() {
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
          ...(expectedVolumes && { expected_volumes: expectedVolumes }),
        }),
      });
      const data = await resp.json();
      if (data.error) {
        setError(data.error);
      } else if (data.success) {
        setSent(true);
        onDownloadStarted?.();
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
    <button
      type="button"
      onClick={handleSend}
      disabled={sending || sent}
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
  );
}

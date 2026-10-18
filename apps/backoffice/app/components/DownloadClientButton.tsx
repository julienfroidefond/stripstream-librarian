"use client";

import { useState, useEffect, useMemo, createContext, useContext, type ReactNode } from "react";
import { Icon, Button, Modal } from "./ui";
import { useTranslation } from "@/lib/i18n/context";
import type { DownloadClient } from "@/lib/api";
import {
  DOWNLOAD_CLIENTS,
  DOWNLOAD_CLIENT_LABELS,
  DOWNLOAD_CLIENT_SETTING_KEY,
  downloadClientAddEndpoint,
  isClientConfigured,
  readDownloadClientSetting,
} from "@/lib/downloadClient";

// Re-exported for convenience: components, tests and callers import them from here.
export * from "@/lib/downloadClient";

interface DownloadClientContextValue {
  /** Clients currently configured, in `DOWNLOAD_CLIENTS` order. */
  available: DownloadClient[];
  /** Active client, or `null` when none is configured. */
  client: DownloadClient | null;
  onDownloadStarted?: () => void;
}

const DownloadClientContext = createContext<DownloadClientContextValue>({
  available: [],
  client: null,
});

export function DownloadClientProvider({
  children,
  initialClients,
  initialClient,
  onDownloadStarted,
}: {
  children: ReactNode;
  /**
   * Server-resolved availability per client. A client missing from the map (or
   * set to `undefined`) is resolved client-side with a settings fetch.
   */
  initialClients?: Partial<Record<DownloadClient, boolean>>;
  /**
   * Client chosen in Settings. When omitted, it is read from the
   * `download_client` setting on mount.
   */
  initialClient?: DownloadClient | null;
  onDownloadStarted?: () => void;
}) {
  const [availability, setAvailability] = useState<Record<DownloadClient, boolean>>(() => ({
    qbittorrent: initialClients?.qbittorrent ?? false,
    alldebrid: initialClients?.alldebrid ?? false,
  }));
  const [preferred, setPreferred] = useState<DownloadClient | null>(initialClient ?? null);
  // Whether the client chosen in Settings is known. When the server did not
  // supply it, it is only known once the settings fetch resolves: until then no
  // client is exposed at all, because falling back to the first configured one
  // would silently send a snatch to the wrong backend.
  const [preferredResolved, setPreferredResolved] = useState(initialClient !== undefined);

  // Resolve any client the server did not tell us about.
  useEffect(() => {
    const unknown = DOWNLOAD_CLIENTS.filter((c) => initialClients?.[c] === undefined);
    if (unknown.length === 0) return;
    let active = true;
    Promise.all(
      unknown.map(async (c) => {
        try {
          const resp = await fetch(`/api/settings/${c}`);
          const data = resp.ok ? await resp.json() : null;
          return [c, isClientConfigured(c, data)] as const;
        } catch {
          return [c, false] as const;
        }
      }),
    ).then((entries) => {
      if (!active) return;
      setAvailability((prev) => {
        const next = { ...prev };
        for (const [c, configured] of entries) next[c] = configured;
        return next;
      });
    });
    return () => {
      active = false;
    };
    // Depend on the primitive entries: `initialClients` is a fresh object each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialClients?.qbittorrent, initialClients?.alldebrid]);

  // Read the client chosen in Settings when the server did not supply it.
  useEffect(() => {
    if (initialClient !== undefined) return;
    let active = true;
    fetch(`/api/settings/${DOWNLOAD_CLIENT_SETTING_KEY}`)
      .then((resp) => (resp.ok ? resp.json() : null))
      .catch(() => null)
      .then((data) => {
        if (!active) return;
        setPreferred(readDownloadClientSetting(data));
        setPreferredResolved(true);
      });
    return () => {
      active = false;
    };
  }, [initialClient]);

  const available = useMemo(() => DOWNLOAD_CLIENTS.filter((c) => availability[c]), [availability]);
  // Fall back to the first available client when the stored choice is unusable,
  // but never before that choice is known (see `preferredResolved`).
  const activeClient = !preferredResolved
    ? null
    : preferred && availability[preferred]
      ? preferred
      : (available[0] ?? null);

  return (
    <DownloadClientContext.Provider value={{ available, client: activeClient, onDownloadStarted }}>
      {children}
    </DownloadClientContext.Provider>
  );
}

/**
 * Read-only indication of the client snatches use.
 *
 * The choice itself lives in Settings — this chip only tells where snatches go.
 * Renders nothing when no client is configured.
 */
export function DownloadClientIndicator() {
  const { t } = useTranslation();
  const { client } = useContext(DownloadClientContext);
  if (!client) return null;

  return (
    <span
      title={t("downloadClient.activeHint")}
      className="inline-flex items-center gap-1.5 text-xs text-muted-foreground"
    >
      <Icon name="download" size="sm" />
      {t("downloadClient.active", { client: DOWNLOAD_CLIENT_LABELS[client] })}
    </span>
  );
}

/** Small "handled by <client>" chip for a download row. */
export function DownloadClientBadge({ client }: { client: DownloadClient }) {
  const label = DOWNLOAD_CLIENT_LABELS[client] ?? client;
  return (
    <span
      title={label}
      className="inline-flex items-center gap-1 px-1.5 py-px rounded text-3xs font-medium bg-muted text-muted-foreground shrink-0"
    >
      <Icon name={client === "alldebrid" ? "globe" : "download"} size="sm" />
      {label}
    </span>
  );
}

export function DownloadClientButton({
  downloadUrl,
  releaseId,
  libraryId,
  seriesName,
  expectedVolumes,
  allVolumes,
  alwaysShowReplace,
  requiresReview = false,
}: {
  downloadUrl: string;
  releaseId: string;
  libraryId?: string;
  seriesName?: string;
  expectedVolumes?: number[];
  allVolumes?: number[];
  /** Show replace button even when allVolumes == expectedVolumes (e.g. in Prowlarr search modal) */
  alwaysShowReplace?: boolean;
  /** Require a confirmation before downloading a weak title match. */
  requiresReview?: boolean;
}) {
  const { t } = useTranslation();
  const { client, onDownloadStarted } = useContext(DownloadClientContext);
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmMode, setConfirmMode] = useState<"replace" | "review" | null>(null);

  if (!client) return null;

  const clientLabel = DOWNLOAD_CLIENT_LABELS[client];

  const showReplaceButton = alwaysShowReplace
    || (allVolumes && allVolumes.length > 0 && expectedVolumes && allVolumes.length > expectedVolumes.length);

  async function handleSend(volumes?: number[], replaceExisting = false) {
    setSending(true);
    setError(null);
    try {
      const vols = volumes || expectedVolumes;
      const resp = await fetch(downloadClientAddEndpoint(client!), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url: downloadUrl,
          ...(libraryId && { library_id: libraryId }),
          ...(seriesName && { series_name: seriesName }),
          ...(vols && vols.length > 0 ? { expected_volumes: vols } : replaceExisting && libraryId ? { expected_volumes: [] } : {}),
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
        setError(data.message || t("downloadClient.sentError", { client: clientLabel }));
      }
    } catch {
      setError(t("downloadClient.sentError", { client: clientLabel }));
    } finally {
      setSending(false);
    }
  }

  return (
    <>
      <div className="inline-flex items-center gap-0.5">
        <button
          type="button"
          onClick={() => requiresReview ? setConfirmMode("review") : handleSend()}
          disabled={sending}
          className={`inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-50 shrink-0 ${
            sent
              ? "text-success"
              : error
                ? "text-destructive"
                : "text-primary hover:bg-primary/10"
          }`}
          title={sent ? t("downloadClient.sentTo", { client: clientLabel }) : error || t("downloadClient.sendTo", { client: clientLabel })}
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
            onClick={() => setConfirmMode("replace")}
            disabled={sending}
            className="inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-50 shrink-0 text-warning hover:bg-warning/10"
            title={t("prowlarr.replaceAndDownload")}
          >
            <Icon name="refresh" size="sm" />
          </button>
        )}
      </div>

      <Modal isOpen={confirmMode !== null} onClose={() => setConfirmMode(null)} maxWidth="sm">
        <div className="p-6">
          <h3 className="text-lg font-semibold text-foreground mb-2">
            {confirmMode === "review" ? t("downloadClient.reviewTitle") : t("prowlarr.replaceAndDownload")}
          </h3>
          <p className="text-sm text-muted-foreground">
            {confirmMode === "review"
              ? t("downloadClient.reviewConfirm", { client: clientLabel })
              : t("prowlarr.confirmReplace")}
          </p>
        </div>
        <div className="flex justify-end gap-2 px-6 pb-6">
          <Button variant="outline" size="sm" onClick={() => setConfirmMode(null)}>
            {t("common.cancel")}
          </Button>
          <Button variant={confirmMode === "review" ? "default" : "destructive"} size="sm" onClick={() => {
            const mode = confirmMode;
            setConfirmMode(null);
            if (mode === "review") handleSend(); else handleSend(allVolumes, true);
          }}>
            {confirmMode === "review" ? t("downloadClient.sendTo", { client: clientLabel }) : t("prowlarr.replaceAndDownload")}
          </Button>
        </div>
      </Modal>
    </>
  );
}

"use client";

import { useState, useEffect, useMemo, createContext, useContext, type ReactNode } from "react";
import { Icon, Button, Modal } from "./ui";
import { useTranslation } from "@/lib/i18n/context";
import type { DownloadClient } from "@/lib/api";

/** Brand names of the supported download clients — never translated. */
export const DOWNLOAD_CLIENT_LABELS: Record<DownloadClient, string> = {
  qbittorrent: "qBittorrent",
  alldebrid: "AllDebrid",
};

/** Stable order used to pick a default client when several are available. */
const CLIENT_ORDER: DownloadClient[] = ["qbittorrent", "alldebrid"];

/** `localStorage` key holding the user's preferred download client. */
const STORAGE_KEY = "sl.downloadClient";

/** Backoffice proxy endpoint accepting a snatch for the given client. */
export function downloadClientAddEndpoint(client: DownloadClient): string {
  return `/api/${client}/add`;
}

/**
 * Decide whether a client is usable from its settings payload.
 *
 * qBittorrent needs a URL and a username; AllDebrid must be enabled and hold a
 * non-empty API key. The API never returns the real AllDebrid key (it is
 * redacted as a non-empty placeholder), so a non-empty string is enough here.
 */
export function isClientConfigured(client: DownloadClient, data: Record<string, unknown> | null): boolean {
  if (!data) return false;
  if (client === "qbittorrent") {
    return !!(typeof data.url === "string" && data.url.trim() && typeof data.username === "string" && data.username.trim());
  }
  return data.enabled === true && typeof data.api_key === "string" && data.api_key.trim().length > 0;
}

interface DownloadClientContextValue {
  /** Clients currently configured, in `CLIENT_ORDER`. */
  available: DownloadClient[];
  /** Active client, or `null` when none is configured. */
  client: DownloadClient | null;
  setClient: (client: DownloadClient) => void;
  onDownloadStarted?: () => void;
}

const DownloadClientContext = createContext<DownloadClientContextValue>({
  available: [],
  client: null,
  setClient: () => {},
});

function readStoredClient(): DownloadClient | null {
  if (typeof window === "undefined") return null;
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return CLIENT_ORDER.includes(stored as DownloadClient) ? (stored as DownloadClient) : null;
  } catch {
    return null;
  }
}

export function DownloadClientProvider({
  children,
  initialClients,
  onDownloadStarted,
}: {
  children: ReactNode;
  /**
   * Server-resolved availability per client. A client missing from the map (or
   * set to `undefined`) is resolved client-side with a settings fetch.
   */
  initialClients?: Partial<Record<DownloadClient, boolean>>;
  onDownloadStarted?: () => void;
}) {
  const [availability, setAvailability] = useState<Record<DownloadClient, boolean>>(() => ({
    qbittorrent: initialClients?.qbittorrent ?? false,
    alldebrid: initialClients?.alldebrid ?? false,
  }));
  const [client, setClientState] = useState<DownloadClient | null>(() => readStoredClient());

  // Resolve any client the server did not tell us about.
  useEffect(() => {
    const unknown = CLIENT_ORDER.filter((c) => initialClients?.[c] === undefined);
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

  const available = useMemo(() => CLIENT_ORDER.filter((c) => availability[c]), [availability]);
  // Fall back to the first available client when the stored choice is unusable.
  const activeClient = client && availability[client] ? client : (available[0] ?? null);

  function setClient(next: DownloadClient) {
    setClientState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      /* ignore private-mode / storage errors */
    }
  }

  return (
    <DownloadClientContext.Provider value={{ available, client: activeClient, setClient, onDownloadStarted }}>
      {children}
    </DownloadClientContext.Provider>
  );
}

/** Segmented control to switch download client — hidden unless several are configured. */
export function DownloadClientSelector() {
  const { t } = useTranslation();
  const { available, client, setClient } = useContext(DownloadClientContext);
  if (available.length < 2) return null;

  return (
    <div
      role="group"
      aria-label={t("downloadClient.select")}
      className="inline-flex items-center rounded-lg border border-border overflow-hidden"
    >
      {available.map((c) => (
        <button
          key={c}
          type="button"
          aria-pressed={client === c}
          onClick={() => setClient(c)}
          className={`px-2.5 py-1 text-xs font-medium transition-colors ${
            client === c ? "bg-primary/15 text-primary" : "bg-card text-muted-foreground hover:text-foreground"
          }`}
        >
          {DOWNLOAD_CLIENT_LABELS[c]}
        </button>
      ))}
    </div>
  );
}

/**
 * Always-visible indication of the client snatches use.
 *
 * Renders the segmented selector when several clients are configured and a
 * static chip when only one is, so the destination is never a surprise. Renders
 * nothing when no client is configured.
 */
export function DownloadClientIndicator() {
  const { t } = useTranslation();
  const { available, client } = useContext(DownloadClientContext);
  if (available.length === 0 || !client) return null;

  if (available.length === 1) {
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

  return <DownloadClientSelector />;
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

"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon, Modal, toast } from "./ui";
import { useTranslation } from "@/lib/i18n/context";
import { stripLeadingArticle } from "../../lib/volumeRanges";
import type { TranslationKey } from "../../lib/i18n/fr";
import type {
  DdlProviderDto,
  DdlProviderDownloadDto,
  DdlProviderSearchResultDto,
} from "../../lib/api";

type TFunction = (key: TranslationKey, vars?: Record<string, string | number>) => string;

interface MissingBookItem {
  title: string | null;
  volume_number: number | null;
}

interface DdlProviderSearchModalProps {
  seriesName: string;
  libraryId: string;
  missingBooks?: MissingBookItem[] | null;
  /** Only consider this volume for match ranking (book detail page). */
  targetVolume?: number | null;
  /** Pre-fill and run the search with this query. Defaults to the series name. */
  initialQuery?: string;
  /** When `false`, the trigger is not rendered (matches the Telegram modal). */
  initialEnabled?: boolean;
  children?: (open: () => void) => React.ReactNode;
}

function statusClass(status: string | null | undefined): string {
  switch (status) {
    case "queued":      return "bg-muted/50 text-muted-foreground";
    case "downloading": return "bg-primary/10 text-primary";
    case "imported":    return "bg-success/10 text-success";
    case "failed":      return "bg-destructive/10 text-destructive";
    case "dismissed":   return "bg-muted/30 text-muted-foreground";
    default:            return "bg-muted/30 text-muted-foreground";
  }
}

function statusLabel(status: string | null | undefined, t: TFunction): string | null {
  if (!status) return null;
  const map: Record<string, TranslationKey> = {
    queued: "ddlProviders.status.queued",
    downloading: "ddlProviders.status.downloading",
    imported: "ddlProviders.status.imported",
    failed: "ddlProviders.status.failed",
    dismissed: "ddlProviders.status.dismissed",
  };
  return map[status] ? t(map[status]) : status;
}

function formatSize(bytes: number | null): string {
  if (!bytes) return "";
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(0) + " KB";
  return bytes + " B";
}

/** `provider:external_id` → download status. */
function downloadKey(provider: string, externalId: string): string {
  return `${provider}:${externalId}`;
}

export function DdlProviderSearchModal({
  seriesName,
  libraryId,
  missingBooks,
  targetVolume,
  initialQuery,
  initialEnabled,
  children,
}: DdlProviderSearchModalProps) {
  const { t } = useTranslation();
  const strippedName = stripLeadingArticle(seriesName);
  const defaultQuery = initialQuery?.replace(/"/g, "").trim() || strippedName;

  const volumeTargets = [
    ...(missingBooks ?? [])
      .map((book) => book.volume_number)
      .filter((n): n is number => n != null),
    ...(targetVolume != null ? [targetVolume] : []),
  ];
  const missingVolumes = Array.from(new Set(volumeTargets));

  const [providers, setProviders] = useState<DdlProviderDto[] | null>(null);
  const [providerId, setProviderId] = useState<string | null>(null);
  const [isOpen, setIsOpen] = useState(false);
  const [searchInput, setSearchInput] = useState(defaultQuery);
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<DdlProviderSearchResultDto[]>([]);
  const [searched, setSearched] = useState(false);
  const [statuses, setStatuses] = useState<Record<string, string>>({});
  const [actingKeys, setActingKeys] = useState<Set<string>>(new Set());

  const enabledProviders = (providers ?? []).filter((p) => p.enabled);

  // Load the DDL provider list once so we know whether to render the trigger and
  // which providers the user can pick from.
  useEffect(() => {
    let cancelled = false;
    fetch("/api/ddl-providers")
      .then((resp) => (resp.ok ? resp.json() : []))
      .then((list: DdlProviderDto[]) => {
        if (cancelled) return;
        setProviders(list);
        const firstEnabled = list.find((p) => p.enabled);
        setProviderId((prev) => prev ?? firstEnabled?.id ?? null);
      })
      .catch(() => {
        if (!cancelled) setProviders([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const refreshStatuses = useCallback(async () => {
    try {
      const resp = await fetch("/api/ddl-provider-downloads");
      if (!resp.ok) return;
      const list: DdlProviderDownloadDto[] = await resp.json();
      setStatuses((prev) => {
        const next = { ...prev };
        for (const d of list) next[downloadKey(d.provider, d.external_id)] = d.status;
        return next;
      });
    } catch {
      /* best-effort */
    }
  }, []);

  const hasActive = Object.values(statuses).some(
    (s) => s === "queued" || s === "downloading"
  );

  useEffect(() => {
    if (!isOpen || !hasActive) return;
    const id = setInterval(refreshStatuses, 3000);
    return () => clearInterval(id);
  }, [isOpen, hasActive, refreshStatuses]);

  const doSearch = useCallback(
    async (query: string, provider: string) => {
      if (!query.trim() || !provider) return;
      setIsSearching(true);
      setSearched(false);
      try {
        const resp = await fetch(`/api/ddl-providers/${provider}/search`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            query: query.trim(),
            series_name: seriesName,
            library_id: libraryId,
            missing_volumes: missingVolumes,
          }),
        });
        if (resp.ok) {
          const data: { results: DdlProviderSearchResultDto[] } = await resp.json();
          setResults(data.results ?? []);
          setStatuses((prev) => {
            const next = { ...prev };
            for (const r of data.results ?? []) {
              if (r.download_status) next[downloadKey(r.provider, r.external_id)] = r.download_status;
            }
            return next;
          });
        } else {
          const body = await resp.json().catch(() => ({}));
          toast(body?.error ?? `Error ${resp.status}`, "error");
        }
        setSearched(true);
      } catch (e) {
        toast(String(e), "error");
        setSearched(true);
      } finally {
        setIsSearching(false);
      }
    },
    [seriesName, libraryId, missingVolumes]
  );

  function handleOpen() {
    setIsOpen(true);
    setSearchInput(defaultQuery);
    setResults([]);
    setSearched(false);
    if (providerId) void doSearch(defaultQuery, providerId);
  }

  function handleClose() {
    setIsOpen(false);
  }

  async function handleDownload(result: DdlProviderSearchResultDto) {
    if (!providerId) return;
    const key = downloadKey(providerId, result.external_id);
    setActingKeys((prev) => new Set(prev).add(key));
    setStatuses((prev) => ({ ...prev, [key]: "queued" }));
    try {
      const resp = await fetch(`/api/ddl-providers/${providerId}/downloads`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          external_id: result.external_id,
          filename: result.filename,
          size: result.size,
          library_id: libraryId,
          // Always import into the series the user is browsing: the provider's own
          // series label ("Astérix le Gaulois") would create a parallel directory/series.
          series_name: seriesName,
          volume_number: result.volume_number,
        }),
      });
      if (!resp.ok) {
        const body = await resp.json().catch(() => ({}));
        toast(body?.error ?? `Error ${resp.status}`, "error");
        setStatuses((prev) => {
          const next = { ...prev };
          delete next[key];
          return next;
        });
      }
    } catch (e) {
      toast(String(e), "error");
      setStatuses((prev) => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
    } finally {
      setActingKeys((prev) => {
        const s = new Set(prev);
        s.delete(key);
        return s;
      });
    }
  }

  async function handleDismiss(result: DdlProviderSearchResultDto) {
    if (!providerId) return;
    const key = downloadKey(providerId, result.external_id);
    setActingKeys((prev) => new Set(prev).add(key));
    try {
      const resp = await fetch(`/api/ddl-provider-downloads/${result.download_id}`, {
        method: "DELETE",
      });
      if (resp.ok) setStatuses((prev) => ({ ...prev, [key]: "dismissed" }));
    } finally {
      setActingKeys((prev) => {
        const s = new Set(prev);
        s.delete(key);
        return s;
      });
    }
  }

  if (initialEnabled === false) return null;
  if (providers !== null && enabledProviders.length === 0) return null;

  const modal = (
    <Modal
      isOpen={isOpen}
      onClose={handleClose}
      maxWidth="5xl"
      title={
        <span className="flex items-center gap-2">
          <Icon name="globe" size="sm" className="text-emerald-500" />
          {t("ddlProviders.searchModalTitle", { series: seriesName })}
        </span>
      }
    >
      <div className="p-5 space-y-4">
        {/* DDL provider selector (only when more than one is enabled) */}
        {enabledProviders.length > 1 && (
          <div className="flex items-center gap-2">
            <label htmlFor="download-provider-select" className="text-xs text-muted-foreground shrink-0">
              {t("ddlProviders.provider")}
            </label>
            <select
              id="download-provider-select"
              value={providerId ?? ""}
              onChange={(e) => {
                setProviderId(e.target.value);
                doSearch(searchInput, e.target.value);
              }}
              className="px-3 py-1.5 rounded-lg border border-border bg-background text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/50"
            >
              {enabledProviders.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Search input */}
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (providerId) doSearch(searchInput, providerId);
          }}
          className="flex items-center gap-2"
        >
          <input
            type="text"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            className="flex-1 px-3 py-2 rounded-lg border border-border bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
            placeholder={t("ddlProviders.searchPlaceholder")}
          />
          <button
            type="submit"
            disabled={isSearching || !searchInput.trim()}
            className="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
          >
            <Icon name="search" size="sm" />
            {t("prowlarr.searchAction")}
          </button>
        </form>

        {/* Quick badges: series name variants + missing volumes */}
        <div className="flex flex-wrap items-center gap-2 max-h-24 overflow-y-auto">
          {defaultQuery !== strippedName && (
            <button
              type="button"
              onClick={() => {
                setSearchInput(defaultQuery);
                if (providerId) doSearch(defaultQuery, providerId);
              }}
              disabled={isSearching}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors border-primary/50 bg-primary/10 text-primary hover:bg-primary/20"
            >
              {defaultQuery}
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              setSearchInput(strippedName);
              if (providerId) doSearch(strippedName, providerId);
            }}
            disabled={isSearching}
            className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors ${
              defaultQuery !== strippedName
                ? "border-primary/30 bg-primary/5 text-primary/70 hover:bg-primary/15"
                : "border-primary/50 bg-primary/10 text-primary hover:bg-primary/20"
            }`}
          >
            {strippedName}
          </button>
          {strippedName !== seriesName && (
            <button
              type="button"
              onClick={() => {
                setSearchInput(seriesName);
                if (providerId) doSearch(seriesName, providerId);
              }}
              disabled={isSearching}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors border-primary/30 bg-primary/5 text-primary/70 hover:bg-primary/15"
            >
              {seriesName}
            </button>
          )}
          {(missingBooks ?? []).map((book, i) => {
            const label = book.title || (book.volume_number != null ? `T${book.volume_number}` : null);
            if (!label) return null;
            const q = book.volume_number != null ? `${strippedName} T${book.volume_number}` : `${strippedName} ${label}`;
            return (
              <button
                key={i}
                type="button"
                onClick={() => {
                  setSearchInput(q);
                  if (providerId) doSearch(q, providerId);
                }}
                disabled={isSearching}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors border-green-500/50 bg-green-500/10 text-green-600 hover:bg-green-500/20"
              >
                {label}
              </button>
            );
          })}
        </div>

        {/* Searching */}
        {isSearching && (
          <div className="flex items-center gap-2 text-muted-foreground text-sm">
            <Icon name="spinner" size="sm" className="animate-spin" />
            {t("ddlProviders.searching")}
          </div>
        )}

        {/* Results */}
        {!isSearching && results.length > 0 && (
          <div>
            <p className="text-xs text-muted-foreground mb-3">
              {t("ddlProviders.resultCount", { count: results.length })}
            </p>
            <div className="overflow-x-auto rounded-lg border border-border">
              <table className="w-full min-w-[720px] text-sm">
                <tbody>
                  {results.map((result) => {
                    const key = downloadKey(result.provider, result.external_id);
                    const status = statuses[key] ?? null;
                    const isMissing = result.matched_missing_volumes.length > 0;
                    const isActing = actingKeys.has(key);
                    return (
                      <tr
                        key={key}
                        className={`border-b border-border/40 last:border-b-0 transition-colors ${
                          isMissing ? "bg-green-500/10 hover:bg-green-500/15" : "hover:bg-muted/20"
                        } ${status === "dismissed" ? "opacity-40" : ""}`}
                      >
                        <td className="px-3 py-2.5 whitespace-nowrap w-12">
                          {result.volume_number != null ? (
                            <span
                              className={`px-1.5 py-0.5 rounded text-xs font-medium tabular-nums ${
                                isMissing ? "bg-green-500/20 text-green-600" : "bg-muted/50 text-muted-foreground"
                              }`}
                            >
                              T{String(result.volume_number).padStart(2, "0")}
                            </span>
                          ) : (
                            <span className="px-1.5 py-0.5 rounded bg-muted/30 text-muted-foreground text-xs">—</span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 min-w-[240px]">
                          <p className="text-sm text-foreground">{result.filename}</p>
                          <div className="flex flex-wrap items-center gap-1 mt-0.5">
                            {result.series_name && result.series_name !== seriesName && (
                              <span className="px-1 py-px rounded bg-warning/10 text-warning text-[10px]">
                                {result.series_name}
                              </span>
                            )}
                            {result.already_owned && (
                              <span className="px-1 py-px rounded bg-muted/40 text-muted-foreground text-[10px]">
                                {t("ddlProviders.owned")}
                              </span>
                            )}
                            {isMissing && (
                              <span className="px-1 py-px rounded bg-green-500/15 text-green-600 text-[10px]">
                                {t("ddlProviders.matchedMissing", {
                                  volumes: result.matched_missing_volumes.join(", "),
                                })}
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="px-3 py-2.5 whitespace-nowrap text-[11px] text-muted-foreground">
                          {formatSize(result.size)}
                        </td>
                        <td className="px-3 py-2.5 whitespace-nowrap">
                          {statusLabel(status, t) && (
                            <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full ${statusClass(status)}`}>
                              {statusLabel(status, t)}
                            </span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 whitespace-nowrap w-16">
                          <div className="flex items-center gap-1">
                            {(status === null || status === "failed" || status === "dismissed") && (
                              <button
                                type="button"
                                onClick={() => handleDownload(result)}
                                disabled={isActing}
                                title={t("ddlProviders.download")}
                                className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-success hover:bg-success/10 transition-colors disabled:opacity-30"
                              >
                                {isActing ? (
                                  <Icon name="spinner" size="sm" className="animate-spin" />
                                ) : (
                                  <Icon name={status === "failed" ? "refresh" : "download"} size="sm" />
                                )}
                              </button>
                            )}
                            {status === "queued" && <Icon name="clock" size="sm" className="text-muted-foreground" />}
                            {status === "downloading" && (
                              <Icon name="spinner" size="sm" className="animate-spin text-primary" />
                            )}
                            {status === "imported" && (
                              <>
                                <Icon name="check" size="sm" className="text-success" />
                                <button
                                  type="button"
                                  onClick={() => handleDownload(result)}
                                  disabled={isActing}
                                  title={t("ddlProviders.retry")}
                                  className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors disabled:opacity-30"
                                >
                                  {isActing ? (
                                    <Icon name="spinner" size="sm" className="animate-spin" />
                                  ) : (
                                    <Icon name="refresh" size="sm" />
                                  )}
                                </button>
                              </>
                            )}
                            {result.download_id && status !== "dismissed" && status !== "imported" && (
                              <button
                                type="button"
                                onClick={() => handleDismiss(result)}
                                disabled={isActing}
                                title={t("ddlProviders.dismiss")}
                                className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-30"
                              >
                                <Icon name="trash" size="sm" />
                              </button>
                            )}
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* No results */}
        {!isSearching && searched && results.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-3 py-8 text-muted-foreground">
            <Icon name="globe" size="xl" className="opacity-20 text-emerald-500" />
            <p className="text-sm">{t("ddlProviders.noResults")}</p>
          </div>
        )}
      </div>
    </Modal>
  );

  return (
    <>
      {children ? (
        children(handleOpen)
      ) : (
        <button
          type="button"
          onClick={handleOpen}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium border border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
        >
          <Icon name="globe" size="sm" className="text-emerald-500" />
          {t("ddlProviders.searchButton")}
        </button>
      )}
      {modal}
    </>
  );
}

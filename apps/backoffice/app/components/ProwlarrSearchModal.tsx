"use client";

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./ui";
import type { ProwlarrRelease, ProwlarrSearchResponse } from "../../lib/api";
import { useTranslation } from "../../lib/i18n/context";

interface MissingBookItem {
  title: string | null;
  volume_number: number | null;
  external_book_id: string | null;
}

interface ProwlarrSearchModalProps {
  seriesName: string;
  missingBooks: MissingBookItem[] | null;
}

function formatSize(bytes: number): string {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(0) + " KB";
  return bytes + " B";
}

export function ProwlarrSearchModal({ seriesName, missingBooks }: ProwlarrSearchModalProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [isConfigured, setIsConfigured] = useState<boolean | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<ProwlarrRelease[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  // qBittorrent state
  const [isQbConfigured, setIsQbConfigured] = useState(false);
  const [sendingGuid, setSendingGuid] = useState<string | null>(null);
  const [sentGuids, setSentGuids] = useState<Set<string>>(new Set());
  const [sendError, setSendError] = useState<string | null>(null);

  // Check if Prowlarr and qBittorrent are configured on mount
  useEffect(() => {
    fetch("/api/settings/prowlarr")
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        setIsConfigured(!!(data && data.api_key && data.api_key.trim()));
      })
      .catch(() => setIsConfigured(false));
    fetch("/api/settings/qbittorrent")
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        setIsQbConfigured(!!(data && data.url && data.url.trim() && data.username && data.username.trim()));
      })
      .catch(() => setIsQbConfigured(false));
  }, []);

  const [searchInput, setSearchInput] = useState(`"${seriesName}"`);

  const doSearch = useCallback(async (queryOverride?: string) => {
    const searchQuery = queryOverride ?? searchInput;
    if (!searchQuery.trim()) return;
    setIsSearching(true);
    setError(null);
    setResults([]);
    try {
      const body = { series_name: seriesName, custom_query: searchQuery.trim() };
      const resp = await fetch("/api/prowlarr/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await resp.json();
      if (data.error) {
        setError(data.error);
      } else {
        const searchResp = data as ProwlarrSearchResponse;
        setResults(searchResp.results);
        setQuery(searchResp.query);
      }
    } catch {
      setError(t("prowlarr.searchError"));
    } finally {
      setIsSearching(false);
    }
  }, [t, seriesName, searchInput]);

  const defaultQuery = `"${seriesName}"`;

  function handleOpen() {
    setIsOpen(true);
    setResults([]);
    setError(null);
    setQuery("");
    setSearchInput(defaultQuery);
    // Auto-search the series on open
    doSearch(defaultQuery);
  }

  function handleClose() {
    setIsOpen(false);
  }

  async function handleSendToQbittorrent(downloadUrl: string, guid: string) {
    setSendingGuid(guid);
    setSendError(null);
    try {
      const resp = await fetch("/api/qbittorrent/add", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: downloadUrl }),
      });
      const data = await resp.json();
      if (data.error) {
        setSendError(data.error);
      } else if (data.success) {
        setSentGuids((prev) => new Set(prev).add(guid));
      } else {
        setSendError(data.message || t("prowlarr.sentError"));
      }
    } catch {
      setSendError(t("prowlarr.sentError"));
    } finally {
      setSendingGuid(null);
    }
  }

  // Don't render button if not configured
  if (isConfigured === false) return null;
  if (isConfigured === null) return null;

  const modal = isOpen
    ? createPortal(
        <>
          <div
            className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50"
            onClick={handleClose}
          />
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-5xl max-h-[90vh] overflow-y-auto animate-in fade-in zoom-in-95 duration-200">
              {/* Header */}
              <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30 sticky top-0 z-10">
                <h3 className="font-semibold text-foreground">{t("prowlarr.modalTitle")}</h3>
                <button type="button" onClick={handleClose}>
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-muted-foreground hover:text-foreground">
                    <path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                  </svg>
                </button>
              </div>

              <div className="p-5 space-y-4">
              {/* Search input */}
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  if (searchInput.trim()) doSearch(searchInput.trim());
                }}
                className="flex items-center gap-2"
              >
                <input
                  type="text"
                  value={searchInput}
                  onChange={(e) => setSearchInput(e.target.value)}
                  className="flex-1 px-3 py-2 rounded-lg border border-border bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
                  placeholder={t("prowlarr.searchPlaceholder")}
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

              {/* Quick search badges */}
              <div className="flex flex-wrap items-center gap-2 max-h-24 overflow-y-auto">
                <button
                  type="button"
                  onClick={() => { setSearchInput(defaultQuery); doSearch(defaultQuery); }}
                  disabled={isSearching}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border border-primary/50 bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors"
                >
                  {seriesName}
                </button>
              {missingBooks && missingBooks.length > 0 && missingBooks.map((book, i) => {
                const label = book.title || `Vol. ${book.volume_number}`;
                const q = book.volume_number != null ? `"${seriesName}" ${book.volume_number}` : `"${seriesName}" ${label}`;
                return (
                  <button
                    key={i}
                    type="button"
                    onClick={() => { setSearchInput(q); doSearch(q); }}
                    disabled={isSearching}
                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border border-border bg-muted/30 hover:bg-muted/50 disabled:opacity-50 transition-colors"
                  >
                    {label}
                  </button>
                );
              })}
              </div>

              {/* Error */}
              {error && (
                <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                  {error}
                </div>
              )}

              {/* Searching indicator */}
              {isSearching && (
                <div className="flex items-center gap-2 text-muted-foreground text-sm">
                  <Icon name="spinner" size="sm" className="animate-spin" />
                  {t("prowlarr.searching")}
                </div>
              )}

              {/* Results */}
              {!isSearching && results.length > 0 && (
                <div>
                  <p className="text-sm text-muted-foreground mb-3">
                    {t("prowlarr.resultCount", { count: results.length, plural: results.length !== 1 ? "s" : "" })}
                    {query && <span className="ml-1 text-xs opacity-70">({query})</span>}
                  </p>
                  <div className="overflow-x-auto rounded-lg border border-border">
                    <table className="w-full text-sm">
                      <thead>
                        <tr className="bg-muted/50 text-left">
                          <th className="px-3 py-2 font-medium text-muted-foreground">{t("prowlarr.columnTitle")}</th>
                          <th className="px-3 py-2 font-medium text-muted-foreground">{t("prowlarr.columnIndexer")}</th>
                          <th className="px-3 py-2 font-medium text-muted-foreground text-right">{t("prowlarr.columnSize")}</th>
                          <th className="px-3 py-2 font-medium text-muted-foreground text-center">{t("prowlarr.columnSeeders")}</th>
                          <th className="px-3 py-2 font-medium text-muted-foreground text-center">{t("prowlarr.columnLeechers")}</th>
                          <th className="px-3 py-2 font-medium text-muted-foreground">{t("prowlarr.columnProtocol")}</th>
                          <th className="px-3 py-2 font-medium text-muted-foreground text-right"></th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-border">
                        {results.map((release, i) => (
                          <tr key={release.guid || i} className="hover:bg-muted/20 transition-colors">
                            <td className="px-3 py-2 max-w-[400px]">
                              <span className="truncate block" title={release.title}>
                                {release.title}
                              </span>
                            </td>
                            <td className="px-3 py-2 text-muted-foreground whitespace-nowrap">
                              {release.indexer || "—"}
                            </td>
                            <td className="px-3 py-2 text-right text-muted-foreground whitespace-nowrap">
                              {release.size > 0 ? formatSize(release.size) : "—"}
                            </td>
                            <td className="px-3 py-2 text-center">
                              {release.seeders != null ? (
                                <span className={release.seeders > 0 ? "text-green-500 font-medium" : "text-muted-foreground"}>
                                  {release.seeders}
                                </span>
                              ) : "—"}
                            </td>
                            <td className="px-3 py-2 text-center text-muted-foreground">
                              {release.leechers != null ? release.leechers : "—"}
                            </td>
                            <td className="px-3 py-2">
                              {release.protocol && (
                                <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                                  release.protocol === "torrent"
                                    ? "bg-blue-500/15 text-blue-600"
                                    : "bg-amber-500/15 text-amber-600"
                                }`}>
                                  {release.protocol}
                                </span>
                              )}
                            </td>
                            <td className="px-3 py-2">
                              <div className="flex items-center justify-end gap-1.5">
                                {isQbConfigured && release.downloadUrl && (
                                  <button
                                    type="button"
                                    onClick={() => handleSendToQbittorrent(release.downloadUrl!, release.guid)}
                                    disabled={sendingGuid === release.guid || sentGuids.has(release.guid)}
                                    className={`inline-flex items-center justify-center w-7 h-7 rounded-md transition-colors disabled:opacity-50 ${
                                      sentGuids.has(release.guid)
                                        ? "text-green-500"
                                        : "text-primary hover:bg-primary/10"
                                    }`}
                                    title={sentGuids.has(release.guid) ? t("prowlarr.sentSuccess") : t("prowlarr.sendToQbittorrent")}
                                  >
                                    {sendingGuid === release.guid ? (
                                      <Icon name="spinner" size="sm" className="animate-spin" />
                                    ) : sentGuids.has(release.guid) ? (
                                      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <path d="M3 8l4 4 6-7" />
                                      </svg>
                                    ) : (
                                      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <path d="M14 8V14H2V2H8M10 2H14V6M14 2L7 9" />
                                      </svg>
                                    )}
                                  </button>
                                )}
                                {release.downloadUrl && (
                                  <a
                                    href={release.downloadUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center justify-center w-7 h-7 rounded-md text-primary hover:bg-primary/10 transition-colors"
                                    title={t("prowlarr.download")}
                                  >
                                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                      <path d="M8 2v8M4 7l4 4 4-4M2 13h12" />
                                    </svg>
                                  </a>
                                )}
                                {release.infoUrl && (
                                  <a
                                    href={release.infoUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-muted/50 transition-colors"
                                    title={t("prowlarr.info")}
                                  >
                                    <Icon name="externalLink" size="sm" />
                                  </a>
                                )}
                              </div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}

              {/* qBittorrent send error */}
              {sendError && (
                <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                  {sendError}
                </div>
              )}

              {/* No results */}
              {!isSearching && !error && query && results.length === 0 && (
                <p className="text-sm text-muted-foreground">{t("prowlarr.noResults")}</p>
              )}
              </div>
            </div>
          </div>
        </>,
        document.body,
      )
    : null;

  return (
    <>
      <button
        type="button"
        onClick={handleOpen}
        className="inline-flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium border border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors"
      >
        <Icon name="search" size="sm" />
        {t("prowlarr.searchButton")}
      </button>
      {modal}
    </>
  );
}

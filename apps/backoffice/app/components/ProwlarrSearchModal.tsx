"use client";

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./ui";
import type { ProwlarrRelease, ProwlarrSearchResponse } from "../../lib/api";
import { useTranslation } from "../../lib/i18n/context";
import { QbittorrentProvider, QbittorrentDownloadButton } from "./QbittorrentDownloadButton";
import { compressVolumes } from "@/lib/volumeRanges";

interface MissingBookItem {
  title: string | null;
  volume_number: number | null;
  external_book_id: string | null;
}

interface ProwlarrSearchModalProps {
  seriesName: string;
  libraryId?: string;
  missingBooks: MissingBookItem[] | null;
  initialProwlarrConfigured?: boolean;
  initialQbConfigured?: boolean;
}

function formatSize(bytes: number): string {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(0) + " KB";
  return bytes + " B";
}

/** Group releases by identical title, preserving order of first occurrence. */
function groupReleasesByTitle<T extends { title: string }>(releases: T[]): { title: string; items: T[] }[] {
  const groups: Map<string, T[]> = new Map();
  for (const r of releases) {
    const key = r.title;
    const existing = groups.get(key);
    if (existing) {
      existing.push(r);
    } else {
      groups.set(key, [r]);
    }
  }
  return Array.from(groups.entries()).map(([title, items]) => ({ title, items }));
}

export function ProwlarrSearchModal({ seriesName, libraryId, missingBooks, initialProwlarrConfigured, initialQbConfigured }: ProwlarrSearchModalProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [isConfigured, setIsConfigured] = useState<boolean | null>(initialProwlarrConfigured ?? null);
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<ProwlarrRelease[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  // qBittorrent state
  const [isQbConfigured, setIsQbConfigured] = useState(initialQbConfigured ?? false);
  const [sortCol, setSortCol] = useState<"title" | "vol" | "seeders" | "size">("seeders");
  const [sortAsc, setSortAsc] = useState(false);

  // Check if Prowlarr and qBittorrent are configured on mount (skip if server provided)
  useEffect(() => {
    if (initialProwlarrConfigured !== undefined && initialQbConfigured !== undefined) return;
    if (initialProwlarrConfigured === undefined) {
      fetch("/api/settings/prowlarr")
        .then((r) => (r.ok ? r.json() : null))
        .then((data) => {
          setIsConfigured(!!(data && data.api_key && data.api_key.trim()));
        })
        .catch(() => setIsConfigured(false));
    }
    if (initialQbConfigured === undefined) {
      fetch("/api/settings/qbittorrent")
        .then((r) => (r.ok ? r.json() : null))
        .then((data) => {
          setIsQbConfigured(!!(data && data.url && data.url.trim() && data.username && data.username.trim()));
        })
        .catch(() => setIsQbConfigured(false));
    }
  }, [initialProwlarrConfigured, initialQbConfigured]);

  const [searchInput, setSearchInput] = useState(`"${seriesName}"`);

  const doSearch = useCallback(async (queryOverride?: string) => {
    const searchQuery = queryOverride ?? searchInput;
    if (!searchQuery.trim()) return;
    setIsSearching(true);
    setError(null);
    setResults([]);
    try {
      const missing_volumes = missingBooks?.map((b) => ({
        volume_number: b.volume_number,
        title: b.title,
      })) ?? undefined;
      const body = { series_name: seriesName, custom_query: searchQuery.trim(), missing_volumes };
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
                <QbittorrentProvider initialConfigured={isQbConfigured}>
                <div>
                  <p className="text-sm text-muted-foreground mb-3">
                    {t("prowlarr.resultCount", { count: results.length, plural: results.length !== 1 ? "s" : "" })}
                    {query && <span className="ml-1 text-xs opacity-70">({query})</span>}
                  </p>
                  <div className="overflow-x-auto rounded-lg border border-border">
                    <table className="w-full text-sm">
                      <thead>
                        <tr className="bg-muted/50 text-left">
                          {([
                            { id: "title" as const, label: t("prowlarr.columnTitle"), align: "" },
                            { id: "vol" as const, label: "Vol.", align: "text-center" },
                            { id: null, label: t("prowlarr.columnIndexer"), align: "" },
                            { id: "size" as const, label: t("prowlarr.columnSize"), align: "text-right" },
                            { id: "seeders" as const, label: t("prowlarr.columnSeeders"), align: "text-center" },
                          ] as const).map((col, i) => (
                            <th
                              key={i}
                              className={`px-3 py-2 font-medium text-muted-foreground ${col.align} ${col.id ? "cursor-pointer hover:text-foreground select-none" : ""}`}
                              onClick={col.id ? () => {
                                if (sortCol === col.id) setSortAsc(!sortAsc);
                                else { setSortCol(col.id); setSortAsc(false); }
                              } : undefined}
                            >
                              {col.label}
                              {col.id && sortCol === col.id && (
                                <span className="ml-0.5 text-[10px]">{sortAsc ? "▲" : "▼"}</span>
                              )}
                            </th>
                          ))}
                          <th className="px-3 py-2 font-medium text-muted-foreground text-right"></th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-border">
                        {[...results].sort((a, b) => {
                          const dir = sortAsc ? 1 : -1;
                          switch (sortCol) {
                            case "title": return dir * a.title.localeCompare(b.title);
                            case "vol": return dir * ((a.allVolumes?.length ?? 0) - (b.allVolumes?.length ?? 0));
                            case "size": return dir * (a.size - b.size);
                            case "seeders": return dir * ((a.seeders ?? 0) - (b.seeders ?? 0));
                            default: return 0;
                          }
                        }).map((release) => {
                          const hasMissing = release.matchedMissingVolumes && release.matchedMissingVolumes.length > 0;
                          return (
                          <tr key={release.guid} className={`transition-colors ${hasMissing ? "bg-green-500/10 hover:bg-green-500/20 border-l-2 border-l-green-500" : "hover:bg-muted/20"}`}>
                            <td className="px-3 py-2 max-w-[400px]">
                              <span className="truncate block" title={release.title}>
                                {release.title}
                              </span>
                              {hasMissing && (
                                <div className="flex flex-wrap items-center gap-1 mt-0.5">
                                  {compressVolumes(release.matchedMissingVolumes!).map((range) => (
                                    <span key={range} className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-green-500/20 text-green-600">
                                      {range}
                                    </span>
                                  ))}
                                </div>
                              )}
                            </td>
                            <td className="px-3 py-2 text-center text-muted-foreground whitespace-nowrap">
                              {release.allVolumes && release.allVolumes.length > 0 ? release.allVolumes.length : "—"}
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
                            <td className="px-3 py-2">
                              <div className="flex items-center justify-end gap-1.5">
                                {release.downloadUrl && (
                                  <QbittorrentDownloadButton
                                    downloadUrl={release.downloadUrl}
                                    releaseId={release.guid}
                                    libraryId={libraryId}
                                    seriesName={seriesName}
                                    expectedVolumes={release.matchedMissingVolumes ?? release.allVolumes}
                                    allVolumes={release.allVolumes}
                                    alwaysShowReplace
                                  />
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
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                </div>
                </QbittorrentProvider>
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
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium border border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
      >
        <Icon name="search" size="sm" />
        {t("prowlarr.searchButton")}
      </button>
      {modal}
    </>
  );
}

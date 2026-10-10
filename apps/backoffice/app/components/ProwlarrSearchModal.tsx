"use client";

import { useState, useEffect, useCallback } from "react";
import { Icon, Modal } from "./ui";
import type { ProwlarrRelease, ProwlarrSearchResponse } from "../../lib/api";
import { useTranslation } from "@/lib/i18n/context";
import { QbittorrentProvider, QbittorrentDownloadButton } from "./QbittorrentDownloadButton";
import { compressVolumes, stripLeadingArticle } from "@/lib/volumeRanges";

interface MissingBookItem {
  title: string | null;
  volume_number: number | null;
  external_book_id: string | null;
}

export interface QuickSearch {
  label: string;
  query: string;
}

interface ProwlarrSearchModalProps {
  seriesName: string;
  libraryId?: string;
  missingBooks: MissingBookItem[] | null;
  initialProwlarrConfigured?: boolean;
  initialQbConfigured?: boolean;
  /** Pre-fill the search input. Defaults to `"${seriesName}"`. */
  initialQuery?: string;
  /** Override the default quick-search badges (series + missing volumes). */
  quickSearches?: QuickSearch[];
  /** Default volumes to associate with downloaded releases (book-detail context). */
  defaultExpectedVolumes?: number[];
  children?: (open: () => void) => React.ReactNode;
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

export function ProwlarrSearchModal({ seriesName, libraryId, missingBooks, initialProwlarrConfigured, initialQbConfigured, initialQuery, quickSearches, defaultExpectedVolumes, children }: ProwlarrSearchModalProps) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [isConfigured, setIsConfigured] = useState<boolean | null>(initialProwlarrConfigured ?? null);
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<ProwlarrRelease[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  // qBittorrent state
  const [isQbConfigured, setIsQbConfigured] = useState(initialQbConfigured ?? false);
  const [sortCol, setSortCol] = useState<"title" | "seeders" | "size">("seeders");
  const [sortAsc, setSortAsc] = useState(false);

  // Check if Prowlarr and qBittorrent are configured on mount (skip if server provided)
  useEffect(() => {
    if (initialProwlarrConfigured !== undefined && initialQbConfigured !== undefined) return;
    if (initialProwlarrConfigured === undefined) {
      fetch("/api/settings/downloads_enabled")
        .then((r) => (r.ok ? r.json() : null))
        .then((data) => {
          setIsConfigured(!!(data?.enabled));
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

  const strippedName = stripLeadingArticle(seriesName);
  const defaultQuery = initialQuery ?? `"${strippedName}"`;
  const [searchInput, setSearchInput] = useState(defaultQuery);

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

  const modal = (
    <Modal isOpen={isOpen} onClose={handleClose} maxWidth="5xl" title={t("prowlarr.modalTitle")}>
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
              {(() => {
                const baseBadges: QuickSearch[] = quickSearches ?? [
                  { label: strippedName, query: defaultQuery },
                  ...(strippedName !== seriesName ? [{ label: seriesName, query: `"${seriesName}"` }] : []),
                  ...((missingBooks ?? []).map((book) => {
                    const label = book.title || `Vol. ${book.volume_number}`;
                    const q = book.volume_number != null
                      ? `"${strippedName}" T${book.volume_number}`
                      : `"${strippedName}" ${label}`;
                    return { label, query: q };
                  })),
                ];
                const badges = baseBadges;
                if (badges.length === 0) return null;
                return (
                  <div className="flex flex-wrap items-center gap-2 max-h-24 overflow-y-auto">
                    {badges.map((b, i) => (
                      <button
                        key={i}
                        type="button"
                        onClick={() => { setSearchInput(b.query); doSearch(b.query); }}
                        disabled={isSearching}
                        className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border disabled:opacity-50 transition-colors ${
                          i === 0
                            ? "border-primary/50 bg-primary/10 text-primary hover:bg-primary/20"
                            : "border-border bg-muted/30 hover:bg-muted/50"
                        }`}
                      >
                        {b.label}
                      </button>
                    ))}
                  </div>
                );
              })()}

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
                    <table className="w-full min-w-[640px] text-sm">
                      <thead>
                        <tr className="bg-muted/50 text-left">
                          {([
                            { id: "title" as const, label: t("prowlarr.columnTitle"), align: "" },
                            { id: null, label: t("prowlarr.columnIndexer"), align: "" },
                            { id: null, label: t("prowlarr.columnCategory"), align: "" },
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
                            case "size": return dir * (a.size - b.size);
                            case "seeders": return dir * ((a.seeders ?? 0) - (b.seeders ?? 0));
                            default: return 0;
                          }
                        }).map((release) => {
                          const hasMissing = release.matchedMissingVolumes && release.matchedMissingVolumes.length > 0;
                          return (
                          <tr key={release.guid} className={`transition-colors ${hasMissing ? "bg-green-500/10 hover:bg-green-500/20 border-l-2 border-l-green-500" : "hover:bg-muted/20"}`}>
                            <td className="px-3 py-2 min-w-[200px] max-w-[400px]">
                              <span className="break-words block">
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
                              {release.matchConfidence === "review" && (
                                <span className="inline-flex mt-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-warning/20 text-warning" title={release.matchReasons?.join(", ")}>
                                  À vérifier
                                </span>
                              )}
                            </td>
                            <td className="px-3 py-2 text-muted-foreground whitespace-nowrap">
                              {release.indexer || "—"}
                            </td>
                            <td className="px-3 py-2">
                              <div className="flex flex-wrap gap-1">
                                {release.categories?.map((cat) => (
                                  <span key={cat.id} title={cat.name ?? undefined} className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground whitespace-nowrap">
                                    {cat.id}
                                  </span>
                                )) ?? "—"}
                              </div>
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
                                    expectedVolumes={release.matchedMissingVolumes ?? release.allVolumes ?? defaultExpectedVolumes}
                                    allVolumes={release.allVolumes}
                                    alwaysShowReplace
                                    requiresReview={release.matchConfidence === "review"}
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
          <Icon name="search" size="sm" />
          {t("prowlarr.searchButton")}
        </button>
      )}
      {modal}
    </>
  );
}

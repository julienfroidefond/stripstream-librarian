"use client";

import { useState, useCallback, useEffect } from "react";
import { Icon, Modal } from "@/app/components/ui";
import { ProviderIcon, PROVIDERS } from "@/app/components/ProviderIcon";
import { useTranslation } from "@/lib/i18n/context";
import { getBookCoverUrl } from "@/lib/api";
import type { SeriesCandidateDto, SeriesDto } from "@/lib/api";

export interface ProwlarrItem {
  series_name: string;
  release_count: number;
  best_seeders: number;
  total_seeders: number;
  categories: string[];
  indexers: string[];
  best_release_title: string;
  best_download_url: string | null;
  best_size: number;
  best_publish_date: string | null;
  best_info_url: string | null;
  best_indexer: string | null;
  volumes_found: number[];
}

interface Library {
  id: string;
  name: string;
}

type Step = "library" | "search" | "adding" | "done" | "error";

function ConfidenceBadge({ confidence }: { confidence: number }) {
  const color =
    confidence >= 0.8
      ? "bg-green-500/10 text-green-600 border-green-500/30"
      : confidence >= 0.5
        ? "bg-yellow-500/10 text-yellow-600 border-yellow-500/30"
        : "bg-red-500/10 text-red-600 border-red-500/30";
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded-full border ${color} shrink-0`}>
      {Math.round(confidence * 100)}%
    </span>
  );
}

export function ProwlarrAddModal({
  item,
  libraries,
  onAdded,
  children,
}: {
  item: ProwlarrItem;
  libraries: Library[];
  onAdded: () => void;
  children: (open: () => void) => React.ReactNode;
}) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState<Step>("library");
  const [libraryId, setLibraryId] = useState("");

  // Search state
  const [searchInput, setSearchInput] = useState(item.series_name);
  const [searchProvider, setSearchProvider] = useState("");
  const [searching, setSearching] = useState(false);
  const [metaCandidates, setMetaCandidates] = useState<SeriesCandidateDto[]>([]);
  const [existingSeries, setExistingSeries] = useState<SeriesDto[]>([]);
  const [searchError, setSearchError] = useState<string | null>(null);

  // Selection: only one of these is active at a time
  const [selectedCandidate, setSelectedCandidate] = useState<SeriesCandidateDto | null>(null);
  const [selectedExisting, setSelectedExisting] = useState<SeriesDto | null>(null);

  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function runSearch(libId: string, query: string, provider?: string) {
    setSearching(true);
    setSearchError(null);
    setMetaCandidates([]);
    setExistingSeries([]);
    setSelectedCandidate(null);
    setSelectedExisting(null);

    const q = query.trim() || item.series_name;

    const [seriesResp, metaResp] = await Promise.allSettled([
      fetch(`/api/series?search=${encodeURIComponent(q)}&library_id=${libId}&limit=5`),
      fetch("/api/metadata/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ library_id: libId, series_name: q, ...(provider ? { provider } : {}) }),
      }),
    ]);

    if (seriesResp.status === "fulfilled" && seriesResp.value.ok) {
      const data = await seriesResp.value.json();
      setExistingSeries(data.items ?? []);
    }

    if (metaResp.status === "fulfilled" && metaResp.value.ok) {
      const data = await metaResp.value.json();
      if (Array.isArray(data)) {
        setMetaCandidates(data);
        if (data.length > 0 && data[0].provider && !provider) setSearchProvider(data[0].provider);
      }
    } else if (metaResp.status === "fulfilled") {
      const data = await metaResp.value.json().catch(() => ({}));
      setSearchError(data?.error ?? t("common.error"));
    }

    setSearching(false);
  }

  const startSearchStep = useCallback((libId: string) => {
    setLibraryId(libId);
    setSearchInput(item.series_name);
    setSearchProvider("");
    setStep("search");
    runSearch(libId, item.series_name);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [item.series_name]);

  const open = useCallback(() => {
    setIsOpen(true);
    setSearchInput(item.series_name);
    setSearchProvider("");
    setMetaCandidates([]);
    setExistingSeries([]);
    setSelectedCandidate(null);
    setSelectedExisting(null);
    setSearchError(null);
    setErrorMessage(null);

    if (libraries.length === 1) {
      startSearchStep(libraries[0].id);
    } else {
      setStep("library");
    }
  }, [item.series_name, libraries, startSearchStep]);

  const close = useCallback(() => setIsOpen(false), []);

  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [isOpen, close]);

  function selectExisting(s: SeriesDto) {
    setSelectedExisting(s);
    setSelectedCandidate(null);
  }

  function selectCandidate(c: SeriesCandidateDto) {
    setSelectedCandidate(selectedCandidate?.external_id === c.external_id ? null : c);
    setSelectedExisting(null);
  }

  function handleProviderClick(providerValue: string) {
    setSearchProvider(providerValue);
    runSearch(libraryId, searchInput, providerValue);
  }

  async function handleConfirm(mode: "existing" | "new-meta" | "no-meta", download = true) {
    setStep("adding");
    setErrorMessage(null);
    try {
      if (mode !== "existing") {
        // Create or add the series
        const addPayload = mode === "new-meta" && selectedCandidate
          ? {
              library_id: libraryId,
              provider: selectedCandidate.provider,
              external_id: selectedCandidate.external_id,
              title: selectedCandidate.title,
              description: selectedCandidate.description ?? null,
              authors: selectedCandidate.authors ?? [],
              publishers: selectedCandidate.publishers ?? [],
              genres: item.categories,
              start_year: selectedCandidate.start_year ?? null,
              total_volumes: selectedCandidate.total_volumes ?? null,
              status: null,
              cover_url: selectedCandidate.cover_url ?? null,
              external_url: selectedCandidate.external_url ?? null,
            }
          : {
              library_id: libraryId,
              provider: "prowlarr",
              external_id: `prowlarr:${item.series_name}`,
              title: item.series_name,
              description: null,
              authors: [],
              publishers: [],
              genres: item.categories,
              start_year: null,
              total_volumes: item.volumes_found.length > 0 ? Math.max(...item.volumes_found) : null,
              status: null,
              cover_url: null,
              external_url: null,
            };

        const addResp = await fetch("/api/discovery/add-to-library", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(addPayload),
        });

        if (!addResp.ok) {
          const err = await addResp.json().catch(() => ({}));
          throw new Error(err?.error || `Error ${addResp.status}`);
        }
      }

      // Launch download
      if (download && item.best_download_url) {
        const seriesName = mode === "existing"
          ? selectedExisting!.name
          : (selectedCandidate?.title ?? item.series_name);

        await fetch("/api/qbittorrent/add", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            url: item.best_download_url,
            library_id: libraryId,
            series_name: seriesName,
            expected_volumes: item.volumes_found,
          }),
        });
      }

      setStep("done");
      onAdded();
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : t("common.error"));
      setStep("error");
    }
  }

  const libraryName = libraries.find((l) => l.id === libraryId)?.name ?? "";

  const title =
    step === "library" ? t("discovery.selectLibrary") :
    step === "search" ? item.series_name :
    step === "adding" ? t("discovery.adding") :
    step === "done" ? t("discovery.added") :
    t("common.error");

  return (
    <>
      {children(open)}
      <Modal isOpen={isOpen} onClose={close} title={title} maxWidth="md">
        <div className="p-4 space-y-3">

          {/* Step: library */}
          {step === "library" && (
            <div className="space-y-2">
              {libraries.map((lib) => (
                <button
                  key={lib.id}
                  onClick={() => startSearchStep(lib.id)}
                  className="w-full text-left px-4 py-3 rounded-lg border border-border hover:border-primary/40 hover:bg-primary/5 transition-colors"
                >
                  <p className="font-medium text-sm">{lib.name}</p>
                </button>
              ))}
            </div>
          )}

          {/* Step: search */}
          {step === "search" && (
            <>
              {/* Library context */}
              {libraryName && (
                <p className="text-xs text-muted-foreground">→ {libraryName}</p>
              )}

              {/* Search row */}
              <div className="flex gap-2">
                <input
                  type="text"
                  value={searchInput}
                  onChange={(e) => setSearchInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") runSearch(libraryId, searchInput, searchProvider || undefined); }}
                  className="flex-1 text-sm border border-border rounded-lg px-3 py-1.5 bg-background"
                />
                <button
                  onClick={() => runSearch(libraryId, searchInput, searchProvider || undefined)}
                  disabled={searching}
                  className="px-3 py-1.5 rounded-lg bg-primary text-primary-foreground text-sm disabled:opacity-50"
                >
                  {searching
                    ? <Icon name="spinner" size="sm" className="animate-spin" />
                    : <Icon name="search" size="sm" />}
                </button>
              </div>

              {/* Provider pills */}
              <div className="flex flex-wrap gap-1">
                {PROVIDERS.map((p) => (
                  <button
                    key={p.value}
                    onClick={() => handleProviderClick(p.value)}
                    className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-lg text-xs border transition-colors ${
                      searchProvider === p.value
                        ? "bg-primary/15 text-primary border-primary/30"
                        : "bg-card text-muted-foreground border-border hover:border-primary/30"
                    }`}
                  >
                    <ProviderIcon provider={p.value} size={10} />
                    {p.label}
                  </button>
                ))}
              </div>

              {searching ? (
                <div className="flex justify-center py-6">
                  <Icon name="spinner" size="lg" className="animate-spin text-muted-foreground" />
                </div>
              ) : (
                <div className="space-y-3 max-h-80 overflow-y-auto pr-1">
                  {/* Existing series section */}
                  {existingSeries.length > 0 && (
                    <div>
                      <p className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1.5">
                        {t("discovery.prowlarrExisting")}
                      </p>
                      <div className="space-y-1">
                        {existingSeries.map((s) => (
                          <button
                            key={s.series_id}
                            onClick={() => selectExisting(s)}
                            className={`w-full text-left flex items-center gap-2.5 px-3 py-2 rounded-lg border transition-colors ${
                              selectedExisting?.series_id === s.series_id
                                ? "border-green-500/50 bg-green-500/10"
                                : "border-border hover:border-green-500/30 hover:bg-green-500/5"
                            }`}
                          >
                            <div className="w-6 h-8 rounded bg-muted/50 shrink-0 overflow-hidden">
                              {(s.first_book_id || s.cover_url) ? (
                                <img
                                  src={s.first_book_id ? getBookCoverUrl(s.first_book_id, s.first_book_updated_at) : s.cover_url!}
                                  alt=""
                                  className="w-full h-full object-cover"
                                />
                              ) : null}
                            </div>
                            <div className="min-w-0 flex-1">
                              <p className="text-sm font-medium truncate">{s.name}</p>
                              <p className="text-[10px] text-muted-foreground">{s.book_count} livre{s.book_count !== 1 ? "s" : ""}</p>
                            </div>
                            {selectedExisting?.series_id === s.series_id && (
                              <Icon name="check" size="sm" className="text-green-600 shrink-0" />
                            )}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Metadata candidates section */}
                  {metaCandidates.length > 0 && (
                    <div>
                      <p className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1.5">
                        {t("discovery.prowlarrNewMeta")}
                      </p>
                      <div className="space-y-1">
                        {metaCandidates.map((c) => (
                          <button
                            key={`${c.provider}-${c.external_id}`}
                            onClick={() => selectCandidate(c)}
                            className={`w-full text-left flex items-center gap-2.5 px-3 py-2 rounded-lg border transition-colors ${
                              selectedCandidate?.external_id === c.external_id
                                ? "border-primary/50 bg-primary/10"
                                : "border-border hover:border-primary/30 hover:bg-muted/40"
                            }`}
                          >
                            {c.cover_url ? (
                              <img src={c.cover_url} alt="" className="w-6 h-8 object-cover rounded shrink-0" />
                            ) : (
                              <div className="w-6 h-8 rounded bg-muted/50 shrink-0" />
                            )}
                            <div className="min-w-0 flex-1">
                              <p className="text-sm font-medium truncate">{c.title}</p>
                              <div className="flex items-center gap-1 mt-0.5">
                                <ProviderIcon provider={c.provider} size={10} />
                                <span className="text-[10px] text-muted-foreground">{c.provider}</span>
                                {c.start_year && <span className="text-[10px] text-muted-foreground">· {c.start_year}</span>}
                              </div>
                            </div>
                            <ConfidenceBadge confidence={c.confidence} />
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {!searching && metaCandidates.length === 0 && existingSeries.length === 0 && (
                    <p className="text-xs text-muted-foreground text-center py-3">{t("discovery.noResults")}</p>
                  )}

                  {searchError && (
                    <p className="text-xs text-red-500">{searchError}</p>
                  )}
                </div>
              )}

              {/* Download info */}
              {item.best_release_title && (
                <div className="rounded-lg bg-muted/40 border border-border px-3 py-2">
                  <p className="text-[10px] text-muted-foreground">{t("discovery.prowlarrBestRelease")}</p>
                  <p className="text-xs truncate" title={item.best_release_title}>{item.best_release_title}</p>
                </div>
              )}

              {/* Actions */}
              {selectedExisting ? (
                <div className="flex gap-2 pt-1">
                  <button
                    onClick={() => setSelectedExisting(null)}
                    className="px-3 py-2 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:border-primary/30 transition-colors"
                  >
                    {t("common.cancel")}
                  </button>
                  <button
                    onClick={() => handleConfirm("existing", true)}
                    className="flex-1 px-3 py-2 rounded-lg bg-green-600 text-white text-xs font-medium"
                  >
                    {t("discovery.prowlarrDownloadTo", { name: selectedExisting.name })}
                  </button>
                </div>
              ) : (
                <div className="flex gap-2 pt-1">
                  <button
                    onClick={() => handleConfirm(selectedCandidate ? "new-meta" : "no-meta", false)}
                    className="flex-1 px-3 py-2 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:border-primary/30 transition-colors"
                  >
                    {t("discovery.prowlarrAddOnly")}
                  </button>
                  <button
                    onClick={() => handleConfirm(selectedCandidate ? "new-meta" : "no-meta", true)}
                    className="flex-1 px-3 py-2 rounded-lg bg-primary text-primary-foreground text-xs font-medium"
                  >
                    {t("discovery.prowlarrAddAndDownload")}
                  </button>
                </div>
              )}
            </>
          )}

          {/* Step: adding */}
          {step === "adding" && (
            <div className="flex flex-col items-center gap-3 py-8">
              <Icon name="spinner" size="lg" className="animate-spin text-primary" />
              <p className="text-sm text-muted-foreground">{t("discovery.adding")}</p>
            </div>
          )}

          {/* Step: done */}
          {step === "done" && (
            <div className="flex flex-col items-center gap-3 py-6 text-center">
              <div className="w-10 h-10 rounded-full bg-green-500/10 flex items-center justify-center">
                <Icon name="check" size="md" className="text-green-600" />
              </div>
              <p className="text-sm font-medium">{t("discovery.prowlarrAddDone")}</p>
              {!item.best_download_url && (
                <p className="text-xs text-muted-foreground">{t("discovery.prowlarrNoDownload")}</p>
              )}
              <button onClick={close} className="mt-1 px-4 py-2 rounded-lg border border-border text-sm hover:bg-muted transition-colors">
                {t("common.close")}
              </button>
            </div>
          )}

          {/* Step: error */}
          {step === "error" && (
            <div className="space-y-3">
              <p className="text-sm text-red-500">{errorMessage}</p>
              <div className="flex gap-2">
                <button onClick={close} className="flex-1 px-3 py-2 rounded-lg border border-border text-sm">{t("common.close")}</button>
                <button onClick={() => setStep("search")} className="flex-1 px-3 py-2 rounded-lg bg-primary text-primary-foreground text-sm">
                  {t("common.previous")}
                </button>
              </div>
            </div>
          )}

        </div>
      </Modal>
    </>
  );
}

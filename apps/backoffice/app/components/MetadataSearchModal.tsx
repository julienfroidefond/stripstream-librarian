"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { useRouter } from "next/navigation";
import { Icon, Modal } from "./ui";
import { ProviderIcon, providerLabel } from "./ProviderIcon";
import type { ExternalMetadataLinkDto, SeriesCandidateDto, MissingBooksDto, SyncReport, MetadataProviderDto } from "../../lib/api";
import { useTranslation } from "../../lib/i18n/context";

const FIELD_KEYS: string[] = [
  "description", "authors", "publishers", "start_year",
  "total_volumes", "status", "summary", "isbn", "publish_date", "language",
];

function formatValue(value: unknown): string {
  if (value == null) return "—";
  if (Array.isArray(value)) return value.join(", ");
  if (typeof value === "string") {
    return value.length > 80 ? value.slice(0, 80) + "…" : value;
  }
  return String(value);
}

interface MetadataSearchModalProps {
  libraryId: string;
  seriesName: string;
  existingLink: ExternalMetadataLinkDto | null;
  links?: ExternalMetadataLinkDto[];
  initialMissing: MissingBooksDto | null;
  initialHiddenProviders?: string[];
  children?: (open: () => void) => React.ReactNode;
}

type ModalStep = "idle" | "searching" | "results" | "confirm" | "syncing" | "done" | "linked";

export function MetadataSearchModal({
  libraryId,
  seriesName,
  existingLink,
  links,
  initialMissing,
  initialHiddenProviders,
  children,
}: MetadataSearchModalProps) {
  const { t } = useTranslation();
  const router = useRouter();

  const fieldLabel = (field: string): string => {
    if (FIELD_KEYS.includes(field)) {
      return t(`field.${field}` as any);
    }
    return field;
  };
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState<ModalStep>("idle");
  const [candidates, setCandidates] = useState<SeriesCandidateDto[]>([]);
  const [selectedCandidate, setSelectedCandidate] = useState<SeriesCandidateDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [linkId, setLinkId] = useState<string | null>(existingLink?.id ?? null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [missing, setMissing] = useState<MissingBooksDto | null>(initialMissing);
  const [showMissingList, setShowMissingList] = useState(false);
  const [syncReport, setSyncReport] = useState<SyncReport | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshDone, setRefreshDone] = useState(false);
  const [unlinking, setUnlinking] = useState(false);

  // Provider selector: empty string = library default
  const [searchProvider, setSearchProvider] = useState("");
  const [activeProvider, setActiveProvider] = useState("");
  const [hiddenProviders] = useState<Set<string>>(new Set(initialHiddenProviders ?? []));
  const [searchInput, setSearchInput] = useState(seriesName);
  const [providers, setProviders] = useState<MetadataProviderDto[]>([]);

  useEffect(() => {
    fetch("/api/metadata/providers")
      .then((response) => response.ok ? response.json() : Promise.reject(new Error("provider list failed")))
      .then((data: MetadataProviderDto[]) => setProviders(data))
      .catch(() => setProviders([]));
  }, []);

  const visibleProviders = providers.filter((p) => !hiddenProviders.has(p.id));
  // Keep a local, optimistically-updated copy of the links so primary/unlink
  // actions feel instant. Re-sync whenever the server props change (e.g. after
  // router.refresh()).
  const propsLinks = useMemo(
    () => links ?? (existingLink ? [existingLink] : []),
    [links, existingLink],
  );
  const [localLinks, setLocalLinks] = useState<ExternalMetadataLinkDto[]>(propsLinks);
  const [prevPropsLinks, setPrevPropsLinks] = useState(propsLinks);
  if (propsLinks !== prevPropsLinks) {
    setPrevPropsLinks(propsLinks);
    setLocalLinks(propsLinks);
  }
  const approvedLinks = localLinks.filter((l) => l.status === "approved");
  const primaryLink = approvedLinks.find((l) => l.is_primary) ?? null;

  const handleOpen = useCallback(() => {
    setIsOpen(true);
    setSearchInput(seriesName);
    if (approvedLinks.length > 0) {
      setStep("linked");
    } else {
      doSearch("", seriesName);
    }
  }, [approvedLinks.length, seriesName]);

  const handleClose = useCallback(() => {
    setIsOpen(false);
    setStep("idle");
    setError(null);
    setCandidates([]);
    setSelectedCandidate(null);
    setShowMissingList(false);
    setSyncReport(null);
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, handleClose]);

  async function doSearch(provider: string, customQuery?: string) {
    setStep("searching");
    setError(null);
    setActiveProvider(provider);
    const queryTerm = customQuery ?? searchInput;
    try {
      const body: Record<string, string> = {
        library_id: libraryId,
        series_name: queryTerm,
      };
      if (provider) body.provider = provider;

      const resp = await fetch("/api/metadata/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await resp.json();
      if (!resp.ok) {
        setError(data.error || t("metadata.searchFailed"));
        setStep("results");
        return;
      }
      setCandidates(data);
      // Update activeProvider from first result (the API returns the actual provider used)
      if (data.length > 0 && data[0].provider) {
        setActiveProvider(data[0].provider);
        if (!provider) setSearchProvider(data[0].provider);
      }
      setStep("results");
    } catch {
      setError(t("common.networkError"));
      setStep("results");
    }
  }

  async function handleSelectCandidate(candidate: SeriesCandidateDto) {
    setSelectedCandidate(candidate);
    setStep("confirm");
  }

  async function handleApprove(syncSeries: boolean, syncBooks: boolean) {
    if (!selectedCandidate) return;
    setStep("syncing");
    setError(null);
    try {
      // Create match — use the provider from the candidate
      const matchResp = await fetch("/api/metadata/match", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          library_id: libraryId,
          series_name: seriesName,
          provider: selectedCandidate.provider,
          external_id: selectedCandidate.external_id,
          external_url: selectedCandidate.external_url,
          confidence: selectedCandidate.confidence,
          title: selectedCandidate.title,
          metadata_json: {
            ...selectedCandidate.metadata_json,
            description: selectedCandidate.description,
            authors: selectedCandidate.authors,
            publishers: selectedCandidate.publishers,
            start_year: selectedCandidate.start_year,
          },
          total_volumes: selectedCandidate.total_volumes,
        }),
      });
      const matchData = await matchResp.json();
      if (!matchResp.ok) {
        setError(matchData.error || t("metadata.linkFailed"));
        setStep("results");
        return;
      }
      const newLinkId = matchData.id;
      setLinkId(newLinkId);

      // Approve
      const approveResp = await fetch("/api/metadata/approve", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          id: newLinkId,
          sync_series: syncSeries,
          sync_books: syncBooks,
        }),
      });
      const approveData = await approveResp.json();
      if (!approveResp.ok) {
        setError(approveData.error || t("metadata.approveFailed"));
        setStep("results");
        return;
      }

      // Store sync report
      if (approveData.report) {
        setSyncReport(approveData.report);
      }

      // Fetch missing books info
      if (syncBooks) {
        try {
          const missingResp = await fetch(`/api/metadata/missing?id=${newLinkId}`);
          if (missingResp.ok) {
            setMissing(await missingResp.json());
          }
        } catch { /* ignore */ }
      }

      setStep("done");
      // Refresh the server-rendered link list so the new provider shows up
      // immediately (the modal derives `approvedLinks` from its `links` prop).
      router.refresh();
    } catch {
      setError(t("common.networkError"));
      setStep("results");
    }
  }

  async function handleSetPrimary(id: string) {
    setBusyId(id);
    setError(null);
    const previous = localLinks;
    // Optimistic: mark this link primary and clear the flag on the others.
    setLocalLinks((prev) => prev.map((l) => ({ ...l, is_primary: l.id === id })));
    try {
      const resp = await fetch(`/api/metadata/links?id=${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ is_primary: true, sync_series: true, sync_books: true }),
      });
      if (resp.ok) {
        router.refresh();
      } else {
        setLocalLinks(previous);
        const body = await resp.json().catch(() => ({}));
        setError(body?.error ?? t("common.networkError"));
      }
    } catch {
      setLocalLinks(previous);
      setError(t("common.networkError"));
    } finally {
      setBusyId(null);
    }
  }

  async function handleUnlink(id: string) {
    setUnlinking(true);
    setError(null);
    const previous = localLinks;
    // Optimistic: drop the link from the list immediately.
    setLocalLinks((prev) => prev.filter((l) => l.id !== id));
    try {
      const resp = await fetch(`/api/metadata/links?id=${id}`, { method: "DELETE" });
      if (resp.ok) {
        if (linkId === id) {
          setLinkId(null);
          setMissing(null);
        }
        router.refresh();
      } else {
        setLocalLinks(previous);
        const body = await resp.json().catch(() => ({}));
        setError(body?.error ?? t("common.networkError"));
      }
    } catch {
      setLocalLinks(previous);
      setError(t("common.networkError"));
    } finally {
      setUnlinking(false);
    }
  }

  function confidenceBadge(confidence: number) {
    const color =
      confidence >= 0.8
        ? "bg-green-500/10 text-green-600 border-green-500/30"
        : confidence >= 0.5
          ? "bg-yellow-500/10 text-yellow-600 border-yellow-500/30"
          : "bg-red-500/10 text-red-600 border-red-500/30";
    return (
      <span className={`text-xs px-2 py-0.5 rounded-full border ${color}`}>
        {Math.round(confidence * 100)}%
      </span>
    );
  }

  const modal = isOpen
    ? (
        <Modal isOpen={isOpen} onClose={handleClose} title={step === "linked" ? t("metadata.metadataLink") : t("metadata.searchExternal")}>
              <div className="p-5 space-y-4">
                {/* Search input + Provider selector — visible during searching & results */}
                {(step === "searching" || step === "results") && (
                  <div className="space-y-3">
                    <form
                      onSubmit={(e) => {
                        e.preventDefault();
                        if (searchInput.trim()) doSearch(activeProvider || searchProvider);
                      }}
                      className="flex items-center gap-2"
                    >
                      <input
                        type="text"
                        value={searchInput}
                        onChange={(e) => setSearchInput(e.target.value)}
                        disabled={step === "searching"}
                        className="flex-1 px-3 py-2 rounded-lg border border-border bg-background text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary disabled:opacity-50"
                        placeholder={t("metadata.searchPlaceholder")}
                      />
                      <button
                        type="submit"
                        disabled={step === "searching" || !searchInput.trim()}
                        className="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
                      >
                        <Icon name="search" size="sm" />
                        {t("common.search")}
                      </button>
                    </form>
                    <div className="flex items-center gap-2">
                      <label className="text-sm text-muted-foreground whitespace-nowrap">{t("metadata.provider")}</label>
                      <div className="flex gap-1 flex-wrap">
                        {visibleProviders.map((p) => (
                          <button
                            key={p.id}
                            type="button"
                            disabled={step === "searching"}
                            onClick={() => {
                              setSearchProvider(p.id);
                              doSearch(p.id);
                            }}
                            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium border transition-colors ${
                              (activeProvider || searchProvider) === p.id
                                ? "border-primary bg-primary/10 text-primary"
                                : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary/50"
                            }`}
                          >
                            <ProviderIcon provider={p.id} size={14} />
                            {p.label}
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>
                )}

                {/* SEARCHING */}
                {step === "searching" && (
                  <div className="flex items-center justify-center py-12">
                    <Icon name="spinner" size="lg" className="animate-spin text-primary" />
                    <span className="ml-3 text-muted-foreground">{t("metadata.searching", { name: searchInput })}</span>
                  </div>
                )}

                {/* ERROR */}
                {error && (
                  <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                    {error}
                  </div>
                )}

                {/* RESULTS */}
                {step === "results" && (
                  <>
                    {candidates.length === 0 && !error ? (
                      <p className="text-muted-foreground text-center py-8">{t("metadata.noResults")}</p>
                    ) : (
                      <div className="space-y-2">
                        <p className="text-sm text-muted-foreground mb-2">
                          {t("metadata.resultCount", { count: candidates.length, plural: candidates.length !== 1 ? "s" : "" })}
                          {activeProvider && (
                            <span className="ml-1 text-xs inline-flex items-center gap-1">{t("common.via")} <ProviderIcon provider={activeProvider} size={12} /> <span className="font-medium">{providerLabel(activeProvider)}</span></span>
                          )}
                        </p>
                        {candidates.map((c, i) => (
                          <button
                            key={i}
                            type="button"
                            onClick={() => handleSelectCandidate(c)}
                            className="w-full text-left px-3 py-2.5 rounded-lg border border-border/60 bg-muted/20 hover:bg-muted/40 hover:border-primary/50 transition-colors"
                          >
                            <div className="flex gap-3 items-start">
                              <div className="w-10 h-14 flex-shrink-0 rounded bg-muted/50 overflow-hidden">
                                {c.cover_url ? (
                                  <img src={c.cover_url} alt={c.title} className="w-full h-full object-cover" />
                                ) : (
                                  <div className="w-full h-full flex items-center justify-center text-muted-foreground/40">
                                    <Icon name="image" size="sm" />
                                  </div>
                                )}
                              </div>
                              <div className="flex-1 min-w-0">
                                <div className="flex items-center gap-2">
                                  <span className="font-medium text-sm text-foreground truncate">{c.title}</span>
                                  {confidenceBadge(c.confidence)}
                                </div>
                                {c.authors.length > 0 && (
                                  <p className="text-xs text-muted-foreground truncate">{c.authors.join(", ")}</p>
                                )}
                                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                  {c.publishers.length > 0 && <span>{c.publishers[0]}</span>}
                                  {c.start_year != null && <span>{c.start_year}</span>}
                                  {c.total_volumes != null && (
                                    <span>
                                      {c.total_volumes} {c.metadata_json?.volume_source === "chapters" ? "ch." : "vol."}
                                    </span>
                                  )}
                                  {c.metadata_json?.status === "RELEASING" && (
                                    <span className="italic text-amber-500">{t("metadata.inProgress")}</span>
                                  )}
                                </div>
                              </div>
                            </div>
                          </button>
                        ))}
                      </div>
                    )}
                  </>
                )}

                {/* CONFIRM */}
                {step === "confirm" && selectedCandidate && (
                  <div className="space-y-4">
                    <div className="p-4 rounded-lg bg-muted/30 border border-border/50">
                      <div className="flex gap-3">
                        {selectedCandidate.cover_url && (
                          <img
                            src={selectedCandidate.cover_url}
                            alt={selectedCandidate.title}
                            className="w-16 h-22 object-cover rounded"
                          />
                        )}
                        <div>
                          <h4 className="font-medium text-foreground">{selectedCandidate.title}</h4>
                          {selectedCandidate.authors.length > 0 && (
                            <p className="text-sm text-muted-foreground">{selectedCandidate.authors.join(", ")}</p>
                          )}
                          {selectedCandidate.total_volumes != null && (
                            <p className="text-sm text-muted-foreground">
                              {selectedCandidate.total_volumes} {selectedCandidate.metadata_json?.volume_source === "chapters" ? t("metadata.chapters") : t("metadata.volumes")}
                              {selectedCandidate.metadata_json?.status === "RELEASING" && <span className="italic text-amber-500 ml-1">({t("metadata.inProgress")})</span>}
                            </p>
                          )}
                          <p className="text-xs text-muted-foreground mt-1 inline-flex items-center gap-1">
                            {t("common.via")} <ProviderIcon provider={selectedCandidate.provider} size={12} /> <span className="font-medium">{providerLabel(selectedCandidate.provider)}</span>
                          </p>
                        </div>
                      </div>
                    </div>

                    <p className="text-sm text-foreground font-medium">{t("metadata.howToSync")}</p>

                    <div className="flex flex-col gap-2">
                      <button
                        type="button"
                        onClick={() => handleApprove(true, false)}
                        className="w-full p-3 rounded-lg border border-border bg-card text-left hover:bg-muted/40 hover:border-primary/50 transition-colors"
                      >
                        <p className="font-medium text-sm text-foreground">{t("metadata.syncSeriesOnly")}</p>
                        <p className="text-xs text-muted-foreground">{t("metadata.syncSeriesOnlyDesc")}</p>
                      </button>
                      <button
                        type="button"
                        onClick={() => handleApprove(true, true)}
                        className="w-full p-3 rounded-lg border border-primary/50 bg-primary/5 text-left hover:bg-primary/10 transition-colors"
                      >
                        <p className="font-medium text-sm text-foreground">{t("metadata.syncSeriesAndBooks")}</p>
                        <p className="text-xs text-muted-foreground">{t("metadata.syncSeriesAndBooksDesc")}</p>
                      </button>
                    </div>

                    <button
                      type="button"
                      onClick={() => { setSelectedCandidate(null); setStep("results"); }}
                      className="text-sm text-muted-foreground hover:text-foreground"
                    >
                      {t("metadata.backToResults")}
                    </button>
                  </div>
                )}

                {/* SYNCING */}
                {step === "syncing" && (
                  <div className="flex items-center justify-center py-12">
                    <Icon name="spinner" size="lg" className="animate-spin text-primary" />
                    <span className="ml-3 text-muted-foreground">{t("metadata.syncingMetadata")}</span>
                  </div>
                )}

                {/* DONE */}
                {step === "done" && (
                  <div className="space-y-4">
                    <div className="p-4 rounded-lg bg-green-500/10 border border-green-500/30">
                      <p className="font-medium text-green-600">{t("metadata.syncSuccess")}</p>
                    </div>

                    {/* Sync Report */}
                    {syncReport && (
                      <div className="space-y-3">
                        {/* Series report */}
                        {syncReport.series && (syncReport.series.fields_updated.length > 0 || syncReport.series.fields_skipped.length > 0) && (
                          <div className="p-3 rounded-lg bg-muted/30 border border-border/50">
                            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">{t("metadata.seriesLabel")}</p>
                            {syncReport.series.fields_updated.length > 0 && (
                              <div className="space-y-1">
                                {syncReport.series.fields_updated.map((f, i) => (
                                  <div key={i} className="flex items-center gap-2 text-xs">
                                    <span className="inline-flex items-center gap-1 text-green-600">
                                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" /></svg>
                                    </span>
                                    <span className="font-medium text-foreground">{fieldLabel(f.field)}</span>
                                    <span className="text-muted-foreground truncate max-w-[200px]">{formatValue(f.new_value)}</span>
                                  </div>
                                ))}
                              </div>
                            )}
                            {syncReport.series.fields_skipped.length > 0 && (
                              <div className="space-y-1 mt-1">
                                {syncReport.series.fields_skipped.map((f, i) => (
                                  <div key={i} className="flex items-center gap-2 text-xs text-amber-500">
                                    <svg className="w-3 h-3 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                                    </svg>
                                    <span className="font-medium">{fieldLabel(f.field)}</span>
                                    <span className="text-muted-foreground">{t("metadata.locked")}</span>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}

                        {/* Books message (e.g. provider has no volume data) */}
                        {syncReport.books_message && (
                          <div className="p-3 rounded-lg bg-amber-500/10 border border-amber-500/30">
                            <p className="text-xs text-amber-600">{syncReport.books_message}</p>
                          </div>
                        )}

                        {/* Books report */}
                        {!syncReport.books_message && (syncReport.books.length > 0 || syncReport.books_unmatched > 0) && (
                          <div className="p-3 rounded-lg bg-muted/30 border border-border/50">
                            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                              {t("metadata.booksLabel")} — {t("metadata.booksMatched", { matched: syncReport.books_matched, plural: syncReport.books_matched !== 1 ? "s" : "" })}{syncReport.books_unmatched > 0 && `, ${t("metadata.booksUnmatched", { count: syncReport.books_unmatched, plural: syncReport.books_unmatched !== 1 ? "s" : "" })}`}
                            </p>
                            {syncReport.books.length > 0 && (
                              <div className="space-y-2 max-h-48 overflow-y-auto">
                                {syncReport.books.map((b, i) => (
                                  <div key={i} className="text-xs">
                                    <p className="font-medium text-foreground">
                                      {b.volume != null && <span className="font-mono text-muted-foreground mr-1.5">#{b.volume}</span>}
                                      {b.title}
                                    </p>
                                    <div className="ml-4 space-y-0.5 mt-0.5">
                                      {b.fields_updated.map((f, j) => (
                                        <p key={j} className="flex items-center gap-1.5 text-green-600">
                                          <svg className="w-2.5 h-2.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" /></svg>
                                          <span className="font-medium">{fieldLabel(f.field)}</span>
                                        </p>
                                      ))}
                                      {b.fields_skipped.map((f, j) => (
                                        <p key={`s${j}`} className="flex items-center gap-1.5 text-amber-500">
                                          <svg className="w-2.5 h-2.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                                          </svg>
                                          <span className="font-medium">{fieldLabel(f.field)}</span>
                                          <span className="text-muted-foreground">{t("metadata.locked")}</span>
                                        </p>
                                      ))}
                                    </div>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    )}

                    {/* Missing books */}
                    {missing && (
                      <div className="space-y-3">
                        <div className="grid grid-cols-3 gap-4 p-4 bg-muted/30 rounded-lg">
                          <div>
                            <p className="text-sm text-muted-foreground">{t("metadata.external")}</p>
                            <p className="text-2xl font-semibold">{missing.total_external}</p>
                          </div>
                          <div>
                            <p className="text-sm text-muted-foreground">{t("metadata.local")}</p>
                            <p className="text-2xl font-semibold">{missing.total_local}</p>
                          </div>
                          <div>
                            <p className="text-sm text-muted-foreground">{t("metadata.missingLabel")}</p>
                            <p className="text-2xl font-semibold text-warning">{missing.missing_count}</p>
                          </div>
                        </div>

                        {missing.missing_books.length > 0 && (
                          <div>
                            <button
                              type="button"
                              onClick={() => setShowMissingList(!showMissingList)}
                              className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                            >
                              <Icon name={showMissingList ? "chevronDown" : "chevronRight"} size="sm" />
                              {t("metadata.missingBooks", { count: missing.missing_count, plural: missing.missing_count !== 1 ? "s" : "" })}
                            </button>
                            {showMissingList && (
                              <div className="mt-2 max-h-40 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                                {missing.missing_books.map((b, i) => (
                                  <p key={i} className="text-muted-foreground truncate">
                                    {b.volume_number != null && <span className="font-mono mr-2">#{b.volume_number}</span>}
                                    {b.title || t("metadata.unknown")}
                                  </p>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    )}

                    <button
                      type="button"
                      onClick={() => { handleClose(); router.refresh(); }}
                      className="w-full p-2.5 rounded-lg bg-primary text-primary-foreground font-medium text-sm hover:bg-primary/90 transition-colors"
                    >
                      {t("common.close")}
                    </button>
                  </div>
                )}

                {/* LINKED (already approved) */}
                {step === "linked" && approvedLinks.length > 0 && (
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <p className="text-sm font-medium text-muted-foreground">{t("metadata.linkedProviders")}</p>
                      {approvedLinks.map((link) => (
                        <div
                          key={link.id}
                          className={`p-4 rounded-lg border ${
                            link.is_primary ? "bg-primary/5 border-primary/30" : "bg-muted/20 border-border"
                          }`}
                        >
                          <div className="flex items-start justify-between gap-3">
                            <div className="min-w-0">
                              <p className="font-medium text-foreground inline-flex items-center gap-1.5 flex-wrap">
                                <ProviderIcon provider={link.provider} size={16} />
                                {providerLabel(link.provider)}
                                {link.is_primary && (
                                  <span className="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded-full bg-primary/15 text-primary border border-primary/30">
                                    {t("metadata.primary")}
                                  </span>
                                )}
                              </p>
                              {link.external_url && (
                                <a
                                  href={link.external_url}
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  className="block mt-1 text-xs text-primary hover:underline"
                                >
                                  {t("metadata.viewExternal")}
                                </a>
                              )}
                            </div>
                            <div className="flex items-center gap-2 shrink-0">
                              {link.confidence != null && confidenceBadge(link.confidence)}
                              {!link.is_primary && (
                                <button
                                  type="button"
                                  disabled={busyId === link.id}
                                  onClick={() => handleSetPrimary(link.id)}
                                  className="px-2.5 py-1 rounded-lg border border-primary/30 bg-primary/5 text-xs font-medium text-primary hover:bg-primary/10 transition-colors disabled:opacity-60"
                                >
                                  {busyId === link.id ? (
                                    <Icon name="spinner" size="sm" className="animate-spin" />
                                  ) : (
                                    t("metadata.setPrimary")
                                  )}
                                </button>
                              )}
                              <button
                                type="button"
                                disabled={busyId === link.id || unlinking}
                                onClick={() => handleUnlink(link.id)}
                                className="px-2.5 py-1 rounded-lg border border-destructive/30 bg-destructive/5 text-xs font-medium text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-60"
                              >
                                {t("metadata.unlink")}
                              </button>
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>

                    {initialMissing && (
                      <div className="grid grid-cols-3 gap-4 p-4 bg-muted/30 rounded-lg">
                        <div>
                          <p className="text-sm text-muted-foreground">{t("metadata.external")}</p>
                          <p className="text-2xl font-semibold">{initialMissing.total_external}</p>
                        </div>
                        <div>
                          <p className="text-sm text-muted-foreground">{t("metadata.local")}</p>
                          <p className="text-2xl font-semibold">{initialMissing.total_local}</p>
                        </div>
                        <div>
                          <p className="text-sm text-muted-foreground">{t("metadata.missingLabel")}</p>
                          <p className="text-2xl font-semibold text-warning">{initialMissing.missing_count}</p>
                        </div>
                      </div>
                    )}

                    {initialMissing && initialMissing.missing_books.length > 0 && (
                      <div>
                        <button
                          type="button"
                          onClick={() => setShowMissingList(!showMissingList)}
                          className="text-sm text-muted-foreground hover:text-foreground flex items-center gap-1"
                        >
                          <Icon name={showMissingList ? "chevronDown" : "chevronRight"} size="sm" />
                          {t("metadata.missingBooks", { count: initialMissing.missing_count, plural: initialMissing.missing_count !== 1 ? "s" : "" })}
                        </button>
                        {showMissingList && (
                          <div className="mt-2 max-h-40 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                            {initialMissing.missing_books.map((b, i) => (
                              <p key={i} className="text-muted-foreground truncate">
                                {b.volume_number != null && <span className="font-mono mr-2">#{b.volume_number}</span>}
                                {b.title || t("metadata.unknown")}
                              </p>
                            ))}
                          </div>
                        )}
                      </div>
                    )}

                    <div className="flex gap-2">
                      <button
                        type="button"
                        onClick={() => { doSearch(""); }}
                        className="flex-1 p-2.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
                      >
                        {t("metadata.addProvider")}
                      </button>
                      <button
                        type="button"
                        disabled={refreshing || !primaryLink}
                        onClick={async () => {
                          const targetId = primaryLink?.id ?? linkId;
                          if (!targetId) return;
                          setRefreshing(true);
                          setRefreshDone(false);
                          try {
                            const resp = await fetch(`/api/metadata/refresh-link/${targetId}`, { method: "POST" });
                            if (resp.ok) {
                              setRefreshDone(true);
                              setTimeout(() => setRefreshDone(false), 3000);
                            }
                          } finally {
                            setRefreshing(false);
                          }
                        }}
                        className={`p-2.5 rounded-lg border text-sm font-medium transition-colors ${
                          refreshDone
                            ? "border-success/30 bg-success/5 text-success"
                            : "border-primary/30 bg-primary/5 text-primary hover:bg-primary/10"
                        }`}
                      >
                        {refreshing ? (
                          <Icon name="spinner" size="sm" className="animate-spin" />
                        ) : refreshDone ? (
                          <Icon name="check" size="sm" />
                        ) : (
                          t("metadata.refreshLink")
                        )}
                      </button>
                    </div>
                  </div>
                )}
              </div>
        </Modal>
      )
    : null;

  return (
    <>
      {children ? (
        children(handleOpen)
      ) : (
        <button
          onClick={handleOpen}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
        >
          <Icon name="search" size="sm" />
          {approvedLinks.length > 0 ? t("metadata.metadataButton") : t("metadata.searchButton")}
        </button>
      )}

      {modal}
    </>
  );
}

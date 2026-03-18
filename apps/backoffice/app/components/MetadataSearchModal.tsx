"use client";

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { useRouter } from "next/navigation";
import { Icon } from "./ui";
import { ProviderIcon, PROVIDERS, providerLabel } from "./ProviderIcon";
import type { ExternalMetadataLinkDto, SeriesCandidateDto, MissingBooksDto, SyncReport } from "../../lib/api";

const FIELD_LABELS: Record<string, string> = {
  description: "Description",
  authors: "Auteurs",
  publishers: "Éditeurs",
  start_year: "Année",
  total_volumes: "Nb volumes",
  summary: "Résumé",
  isbn: "ISBN",
  publish_date: "Date de publication",
  language: "Langue",
};

function fieldLabel(field: string): string {
  return FIELD_LABELS[field] ?? field;
}

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
  initialMissing: MissingBooksDto | null;
}

type ModalStep = "idle" | "searching" | "results" | "confirm" | "syncing" | "done" | "linked";

export function MetadataSearchModal({
  libraryId,
  seriesName,
  existingLink,
  initialMissing,
}: MetadataSearchModalProps) {
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState<ModalStep>("idle");
  const [candidates, setCandidates] = useState<SeriesCandidateDto[]>([]);
  const [selectedCandidate, setSelectedCandidate] = useState<SeriesCandidateDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [linkId, setLinkId] = useState<string | null>(existingLink?.id ?? null);
  const [missing, setMissing] = useState<MissingBooksDto | null>(initialMissing);
  const [showMissingList, setShowMissingList] = useState(false);
  const [syncReport, setSyncReport] = useState<SyncReport | null>(null);

  // Provider selector: empty string = library default
  const [searchProvider, setSearchProvider] = useState("");
  const [activeProvider, setActiveProvider] = useState("");

  const handleOpen = useCallback(() => {
    setIsOpen(true);
    if (existingLink && existingLink.status === "approved") {
      setStep("linked");
    } else {
      doSearch("");
    }
  }, [existingLink]);

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

  async function doSearch(provider: string) {
    setStep("searching");
    setError(null);
    setActiveProvider(provider);
    try {
      const body: Record<string, string> = {
        library_id: libraryId,
        series_name: seriesName,
      };
      if (provider) body.provider = provider;

      const resp = await fetch("/api/metadata/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await resp.json();
      if (!resp.ok) {
        setError(data.error || "Search failed");
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
      setError("Network error");
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
        setError(matchData.error || "Failed to create match");
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
        setError(approveData.error || "Failed to approve");
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
    } catch {
      setError("Network error");
      setStep("results");
    }
  }

  async function handleUnlink() {
    if (!linkId) return;
    try {
      const resp = await fetch(`/api/metadata/links?id=${linkId}`, { method: "DELETE" });
      if (resp.ok) {
        setLinkId(null);
        setMissing(null);
        handleClose();
        router.refresh();
      }
    } catch { /* ignore */ }
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
    ? createPortal(
        <>
          <div
            className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50"
            onClick={handleClose}
          />
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-2xl max-h-[90vh] overflow-y-auto animate-in fade-in zoom-in-95 duration-200">
              {/* Header */}
              <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30 sticky top-0 z-10">
                <h3 className="font-semibold text-foreground">
                  {step === "linked" ? "Metadata Link" : "Search External Metadata"}
                </h3>
                <button type="button" onClick={handleClose}>
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-muted-foreground hover:text-foreground">
                    <path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                  </svg>
                </button>
              </div>

              <div className="p-5 space-y-4">
                {/* Provider selector — visible during searching & results */}
                {(step === "searching" || step === "results") && (
                  <div className="flex items-center gap-2">
                    <label className="text-sm text-muted-foreground whitespace-nowrap">Provider :</label>
                    <div className="flex gap-1 flex-wrap">
                      {PROVIDERS.map((p) => (
                        <button
                          key={p.value}
                          type="button"
                          disabled={step === "searching"}
                          onClick={() => {
                            setSearchProvider(p.value);
                            doSearch(p.value);
                          }}
                          className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium border transition-colors ${
                            (activeProvider || searchProvider) === p.value
                              ? "border-primary bg-primary/10 text-primary"
                              : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary/50"
                          }`}
                        >
                          <ProviderIcon provider={p.value} size={14} />
                          {p.label}
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {/* SEARCHING */}
                {step === "searching" && (
                  <div className="flex items-center justify-center py-12">
                    <Icon name="spinner" size="lg" className="animate-spin text-primary" />
                    <span className="ml-3 text-muted-foreground">Searching for &quot;{seriesName}&quot;...</span>
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
                      <p className="text-muted-foreground text-center py-8">No results found.</p>
                    ) : (
                      <div className="space-y-2">
                        <p className="text-sm text-muted-foreground mb-2">
                          {candidates.length} result{candidates.length !== 1 ? "s" : ""} found
                          {activeProvider && (
                            <span className="ml-1 text-xs inline-flex items-center gap-1">via <ProviderIcon provider={activeProvider} size={12} /> <span className="font-medium">{providerLabel(activeProvider)}</span></span>
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
                                  {c.total_volumes != null && <span>{c.total_volumes} vol.</span>}
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
                          {selectedCandidate.total_volumes && (
                            <p className="text-sm text-muted-foreground">{selectedCandidate.total_volumes} volumes</p>
                          )}
                          <p className="text-xs text-muted-foreground mt-1 inline-flex items-center gap-1">
                            via <ProviderIcon provider={selectedCandidate.provider} size={12} /> <span className="font-medium">{providerLabel(selectedCandidate.provider)}</span>
                          </p>
                        </div>
                      </div>
                    </div>

                    <p className="text-sm text-foreground font-medium">How would you like to sync?</p>

                    <div className="flex flex-col gap-2">
                      <button
                        type="button"
                        onClick={() => handleApprove(true, false)}
                        className="w-full p-3 rounded-lg border border-border bg-card text-left hover:bg-muted/40 hover:border-primary/50 transition-colors"
                      >
                        <p className="font-medium text-sm text-foreground">Sync series metadata only</p>
                        <p className="text-xs text-muted-foreground">Update description, authors, publishers, and year</p>
                      </button>
                      <button
                        type="button"
                        onClick={() => handleApprove(true, true)}
                        className="w-full p-3 rounded-lg border border-primary/50 bg-primary/5 text-left hover:bg-primary/10 transition-colors"
                      >
                        <p className="font-medium text-sm text-foreground">Sync series + books</p>
                        <p className="text-xs text-muted-foreground">Also fetch book list and show missing volumes</p>
                      </button>
                    </div>

                    <button
                      type="button"
                      onClick={() => { setSelectedCandidate(null); setStep("results"); }}
                      className="text-sm text-muted-foreground hover:text-foreground"
                    >
                      Back to results
                    </button>
                  </div>
                )}

                {/* SYNCING */}
                {step === "syncing" && (
                  <div className="flex items-center justify-center py-12">
                    <Icon name="spinner" size="lg" className="animate-spin text-primary" />
                    <span className="ml-3 text-muted-foreground">Syncing metadata...</span>
                  </div>
                )}

                {/* DONE */}
                {step === "done" && (
                  <div className="space-y-4">
                    <div className="p-4 rounded-lg bg-green-500/10 border border-green-500/30">
                      <p className="font-medium text-green-600">Metadata synced successfully!</p>
                    </div>

                    {/* Sync Report */}
                    {syncReport && (
                      <div className="space-y-3">
                        {/* Series report */}
                        {syncReport.series && (syncReport.series.fields_updated.length > 0 || syncReport.series.fields_skipped.length > 0) && (
                          <div className="p-3 rounded-lg bg-muted/30 border border-border/50">
                            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">Série</p>
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
                                    <span className="text-muted-foreground">locked</span>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}

                        {/* Books report */}
                        {(syncReport.books.length > 0 || syncReport.books_unmatched > 0) && (
                          <div className="p-3 rounded-lg bg-muted/30 border border-border/50">
                            <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                              Livres — {syncReport.books_matched} matched{syncReport.books_unmatched > 0 && `, ${syncReport.books_unmatched} unmatched`}
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
                                          <span className="text-muted-foreground">locked</span>
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
                            <p className="text-sm text-muted-foreground">External</p>
                            <p className="text-2xl font-semibold">{missing.total_external}</p>
                          </div>
                          <div>
                            <p className="text-sm text-muted-foreground">Local</p>
                            <p className="text-2xl font-semibold">{missing.total_local}</p>
                          </div>
                          <div>
                            <p className="text-sm text-muted-foreground">Missing</p>
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
                              {missing.missing_count} missing book{missing.missing_count !== 1 ? "s" : ""}
                            </button>
                            {showMissingList && (
                              <div className="mt-2 max-h-40 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                                {missing.missing_books.map((b, i) => (
                                  <p key={i} className="text-muted-foreground truncate">
                                    {b.volume_number != null && <span className="font-mono mr-2">#{b.volume_number}</span>}
                                    {b.title || "Unknown"}
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
                      Close
                    </button>
                  </div>
                )}

                {/* LINKED (already approved) */}
                {step === "linked" && existingLink && (
                  <div className="space-y-4">
                    <div className="p-4 rounded-lg bg-primary/5 border border-primary/30">
                      <div className="flex items-center justify-between">
                        <div>
                          <p className="font-medium text-foreground inline-flex items-center gap-1.5">
                            Linked to <ProviderIcon provider={existingLink.provider} size={16} /> {providerLabel(existingLink.provider)}
                          </p>
                          {existingLink.external_url && (
                            <a
                              href={existingLink.external_url}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="block mt-1 text-xs text-primary hover:underline"
                            >
                              View on external source
                            </a>
                          )}
                        </div>
                        {existingLink.confidence != null && confidenceBadge(existingLink.confidence)}
                      </div>
                    </div>

                    {initialMissing && (
                      <div className="grid grid-cols-3 gap-4 p-4 bg-muted/30 rounded-lg">
                        <div>
                          <p className="text-sm text-muted-foreground">External</p>
                          <p className="text-2xl font-semibold">{initialMissing.total_external}</p>
                        </div>
                        <div>
                          <p className="text-sm text-muted-foreground">Local</p>
                          <p className="text-2xl font-semibold">{initialMissing.total_local}</p>
                        </div>
                        <div>
                          <p className="text-sm text-muted-foreground">Missing</p>
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
                          {initialMissing.missing_count} missing book{initialMissing.missing_count !== 1 ? "s" : ""}
                        </button>
                        {showMissingList && (
                          <div className="mt-2 max-h-40 overflow-y-auto p-3 bg-muted/20 rounded-lg text-sm space-y-1">
                            {initialMissing.missing_books.map((b, i) => (
                              <p key={i} className="text-muted-foreground truncate">
                                {b.volume_number != null && <span className="font-mono mr-2">#{b.volume_number}</span>}
                                {b.title || "Unknown"}
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
                        Search again
                      </button>
                      <button
                        type="button"
                        onClick={handleUnlink}
                        className="p-2.5 rounded-lg border border-destructive/30 bg-destructive/5 text-sm font-medium text-destructive hover:bg-destructive/10 transition-colors"
                      >
                        Unlink
                      </button>
                    </div>
                  </div>
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
        onClick={handleOpen}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
      >
        <Icon name="search" size="sm" />
        {existingLink && existingLink.status === "approved" ? "Metadata" : "Search metadata"}
      </button>

      {/* Inline badge when linked */}
      {existingLink && existingLink.status === "approved" && initialMissing && initialMissing.missing_count > 0 && (
        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-yellow-500/10 text-yellow-600 text-xs border border-yellow-500/30">
          {initialMissing.missing_count} missing
        </span>
      )}

      {existingLink && existingLink.status === "approved" && (
        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs border border-primary/30">
          <ProviderIcon provider={existingLink.provider} size={12} />
          <span>{providerLabel(existingLink.provider)}</span>
        </span>
      )}

      {modal}
    </>
  );
}

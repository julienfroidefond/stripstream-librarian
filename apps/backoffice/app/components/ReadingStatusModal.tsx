"use client";

import { useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { useRouter } from "next/navigation";
import { Button } from "./ui";
import { useTranslation } from "../../lib/i18n/context";
import type { AnilistMediaResultDto, AnilistSeriesLinkDto } from "../../lib/api";

interface ReadingStatusModalProps {
  libraryId: string;
  seriesName: string;
  readingStatusProvider: string | null;
  existingLink: AnilistSeriesLinkDto | null;
}

type ModalStep = "idle" | "searching" | "results" | "linked";

export function ReadingStatusModal({
  libraryId,
  seriesName,
  readingStatusProvider,
  existingLink,
}: ReadingStatusModalProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState<ModalStep>(existingLink ? "linked" : "idle");
  const [query, setQuery] = useState(seriesName);
  const [candidates, setCandidates] = useState<AnilistMediaResultDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [link, setLink] = useState<AnilistSeriesLinkDto | null>(existingLink);
  const [isLinking, setIsLinking] = useState(false);
  const [isUnlinking, setIsUnlinking] = useState(false);

  const handleOpen = useCallback(() => {
    setIsOpen(true);
    setStep(link ? "linked" : "idle");
    setQuery(seriesName);
    setCandidates([]);
    setError(null);
  }, [link, seriesName]);

  const handleClose = useCallback(() => setIsOpen(false), []);

  async function handleSearch() {
    setStep("searching");
    setError(null);
    try {
      const resp = await fetch("/api/anilist/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query }),
      });
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Search failed");
      setCandidates(data);
      setStep("results");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Search failed");
      setStep("idle");
    }
  }

  async function handleLink(candidate: AnilistMediaResultDto) {
    setIsLinking(true);
    setError(null);
    try {
      const resp = await fetch(
        `/api/anilist/series/${libraryId}/${encodeURIComponent(seriesName)}`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ anilist_id: candidate.id }),
        }
      );
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Link failed");
      setLink(data);
      setStep("linked");
      router.refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Link failed");
    } finally {
      setIsLinking(false);
    }
  }

  async function handleUnlink() {
    setIsUnlinking(true);
    setError(null);
    try {
      const resp = await fetch(
        `/api/anilist/series/${libraryId}/${encodeURIComponent(seriesName)}`,
        { method: "DELETE" }
      );
      if (!resp.ok) throw new Error("Unlink failed");
      setLink(null);
      setStep("idle");
      router.refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unlink failed");
    } finally {
      setIsUnlinking(false);
    }
  }

  if (!readingStatusProvider) return null;

  const providerLabel = readingStatusProvider === "anilist" ? "AniList" : readingStatusProvider;

  return (
    <>
      <button
        type="button"
        onClick={handleOpen}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
        </svg>
        {t("readingStatus.button")}
      </button>

      {isOpen && createPortal(
        <>
          <div className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50" onClick={handleClose} />
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-200">
              {/* Header */}
              <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30">
                <div className="flex items-center gap-2.5">
                  <svg className="w-5 h-5 text-cyan-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                  </svg>
                  <span className="font-semibold text-lg">{providerLabel} — {seriesName}</span>
                </div>
                <button type="button" onClick={handleClose} className="text-muted-foreground hover:text-foreground transition-colors p-1.5 hover:bg-accent rounded-lg">
                  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              <div className="p-6 space-y-4 max-h-[70vh] overflow-y-auto">
                {/* Linked state */}
                {step === "linked" && link && (
                  <div className="space-y-4">
                    <div className="flex items-start gap-3 p-3 rounded-lg bg-muted/40">
                      <div className="flex-1 min-w-0">
                        <p className="font-medium">{link.anilist_title ?? seriesName}</p>
                        {link.anilist_url && (
                          <a href={link.anilist_url} target="_blank" rel="noopener noreferrer" className="text-xs text-primary hover:underline">
                            {link.anilist_url}
                          </a>
                        )}
                        <p className="text-xs text-muted-foreground mt-1">
                          ID: {link.anilist_id} · {t(`readingStatus.status.${link.status}` as any) || link.status}
                        </p>
                      </div>
                      <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-cyan-500/15 text-cyan-600 shrink-0">
                        {providerLabel}
                      </span>
                    </div>
                    <div className="flex gap-2">
                      <Button variant="secondary" size="sm" onClick={() => setStep("idle")}>
                        {t("readingStatus.changeLink")}
                      </Button>
                      <Button variant="ghost" size="sm" onClick={handleUnlink} disabled={isUnlinking} className="text-destructive hover:text-destructive">
                        {isUnlinking ? t("common.loading") : t("readingStatus.unlink")}
                      </Button>
                    </div>
                  </div>
                )}

                {/* Search form */}
                {(step === "idle" || step === "results") && (
                  <div className="space-y-3">
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        onKeyDown={(e) => { if (e.key === "Enter") handleSearch(); }}
                        placeholder={t("readingStatus.searchPlaceholder")}
                        className="flex-1 text-sm border border-border rounded-lg px-3 py-2 bg-background focus:outline-none focus:ring-2 focus:ring-ring"
                      />
                      <Button onClick={handleSearch} size="sm">
                        {t("readingStatus.search")}
                      </Button>
                    </div>

                    {step === "results" && candidates.length === 0 && (
                      <p className="text-sm text-muted-foreground">{t("readingStatus.noResults")}</p>
                    )}

                    {step === "results" && candidates.length > 0 && (
                      <div className="space-y-2">
                        {candidates.map((c) => (
                          <div key={c.id} className="flex items-center gap-3 p-3 rounded-lg border border-border/60 hover:bg-muted/30 transition-colors">
                            <div className="flex-1 min-w-0">
                              <p className="font-medium text-sm truncate">{c.title_romaji ?? c.title_english}</p>
                              {c.title_english && c.title_english !== c.title_romaji && (
                                <p className="text-xs text-muted-foreground truncate">{c.title_english}</p>
                              )}
                              <div className="flex items-center gap-2 mt-0.5">
                                {c.volumes && <span className="text-xs text-muted-foreground">{c.volumes} vol.</span>}
                                {c.status && <span className="text-xs text-muted-foreground">{c.status}</span>}
                                <a href={c.site_url} target="_blank" rel="noopener noreferrer" className="text-xs text-primary hover:underline">↗</a>
                              </div>
                            </div>
                            <Button size="sm" onClick={() => handleLink(c)} disabled={isLinking} className="shrink-0">
                              {t("readingStatus.link")}
                            </Button>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}

                {/* Searching spinner */}
                {step === "searching" && (
                  <div className="flex items-center gap-2 py-4 text-muted-foreground">
                    <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                    <span className="text-sm">{t("readingStatus.searching")}</span>
                  </div>
                )}

                {error && <p className="text-sm text-destructive">{error}</p>}
              </div>
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  );
}

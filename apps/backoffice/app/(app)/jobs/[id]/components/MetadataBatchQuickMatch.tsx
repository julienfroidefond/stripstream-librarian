"use client";

import { useState } from "react";
import Link from "next/link";
import { Icon, Button } from "@/app/components/ui";
import { ProviderIcon, providerLabel } from "@/app/components/ProviderIcon";
import { useTranslation } from "@/lib/i18n/context";
import type { MetadataBatchResultDto } from "@/lib/api";
import { useRouter } from "next/navigation";

interface CandidateInfo {
  title: string;
  external_id: string;
  external_url?: string | null;
  authors?: string[];
  description?: string | null;
  cover_url?: string | null;
  total_volumes?: number | null;
  start_year?: number | null;
  confidence?: number;
}

export function MetadataBatchQuickMatch({
  results,
  libraryId,
}: {
  results: MetadataBatchResultDto[];
  libraryId: string | null;
}) {
  const { t } = useTranslation();
  const router = useRouter();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [linking, setLinking] = useState<string | null>(null);
  const [linked, setLinked] = useState<Set<string>>(new Set());

  const actionable = results.filter(
    (r) =>
      (r.status === "low_confidence" || r.status === "too_many_results") &&
      r.best_candidate_json &&
      !linked.has(r.id)
  );
  const others = results.filter(
    (r) =>
      !(
        (r.status === "low_confidence" || r.status === "too_many_results") &&
        r.best_candidate_json
      ) && !linked.has(r.id)
  );

  async function handleLink(result: MetadataBatchResultDto) {
    if (!libraryId || !result.best_candidate_json || !result.provider_used) return;
    const candidate = result.best_candidate_json as unknown as CandidateInfo;
    setLinking(result.id);
    try {
      // Create match
      const matchResp = await fetch("/api/metadata/match", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          library_id: libraryId,
          series_name: result.series_name,
          provider: result.provider_used,
          external_id: candidate.external_id,
          external_url: candidate.external_url,
          confidence: candidate.confidence ?? result.best_confidence,
          title: candidate.title,
          metadata_json: {
            description: candidate.description,
            authors: candidate.authors,
            start_year: candidate.start_year,
          },
          total_volumes: candidate.total_volumes,
        }),
      });
      const matchData = await matchResp.json();
      if (!matchResp.ok) return;

      // Approve with series sync
      await fetch("/api/metadata/approve", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          id: matchData.id,
          sync_series: true,
          sync_books: true,
        }),
      });

      setLinked((prev) => new Set(prev).add(result.id));
      router.refresh();
    } finally {
      setLinking(null);
    }
  }

  function confidenceBadge(confidence: number) {
    const color =
      confidence >= 0.8
        ? "text-green-600 bg-green-500/10 border-green-500/30"
        : confidence >= 0.5
          ? "text-yellow-600 bg-yellow-500/10 border-yellow-500/30"
          : "text-red-600 bg-red-500/10 border-red-500/30";
    return (
      <span className={`text-xs px-2 py-0.5 rounded-full border ${color}`}>
        {Math.round(confidence * 100)}%
      </span>
    );
  }

  if (results.length === 0) return null;

  return (
    <div className="lg:col-span-2 space-y-4">
      {/* Actionable results (low_confidence / too_many_results with a candidate) */}
      {actionable.length > 0 && (
        <div className="border border-border rounded-xl overflow-hidden">
          <div className="px-4 py-3 bg-warning/10 border-b border-border/40">
            <h3 className="font-semibold text-sm text-foreground">{t("jobDetail.reviewNeeded")}</h3>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t("jobDetail.reviewNeededDesc", { count: actionable.length })}
            </p>
          </div>
          {actionable.map((r) => {
            const candidate = r.best_candidate_json as unknown as CandidateInfo | null;
            const isExpanded = expandedId === r.id;
            const isLinking = linking === r.id;

            return (
              <div key={r.id} className="border-b border-border/40 last:border-b-0">
                <button
                  type="button"
                  onClick={() => setExpandedId(isExpanded ? null : r.id)}
                  className="w-full flex items-center gap-2 px-4 py-2.5 text-left hover:bg-muted/30 transition-colors"
                >
                  <Icon name={isExpanded ? "chevronDown" : "chevronRight"} size="sm" className="text-muted-foreground shrink-0 !w-3.5 !h-3.5" />
                  <Link
                    href={`/series/${r.series_id ?? ""}`}
                    onClick={(e) => e.stopPropagation()}
                    className="text-sm font-medium text-primary hover:underline truncate"
                  >
                    {r.series_name}
                  </Link>
                  <div className="flex items-center gap-2 ml-auto shrink-0">
                    {r.provider_used && (
                      <ProviderIcon provider={r.provider_used} size={14} />
                    )}
                    {r.best_confidence != null && confidenceBadge(r.best_confidence)}
                    <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                      r.status === "low_confidence" ? "bg-amber-500/15 text-amber-600" : "bg-amber-500/15 text-amber-600"
                    }`}>
                      {r.status === "low_confidence" ? t("jobDetail.lowConfidence") : t("jobDetail.tooManyResults")}
                    </span>
                  </div>
                </button>

                {isExpanded && candidate && (
                  <div className="px-4 py-3 pl-10 bg-muted/10 border-t border-border/20">
                    <div className="flex gap-3">
                      {candidate.cover_url && (
                        <img
                          src={candidate.cover_url}
                          alt={candidate.title}
                          className="w-12 h-16 object-cover rounded shrink-0"
                        />
                      )}
                      <div className="flex-1 min-w-0">
                        <p className="font-medium text-sm text-foreground">{candidate.title}</p>
                        {candidate.authors && candidate.authors.length > 0 && (
                          <p className="text-xs text-muted-foreground">{candidate.authors.join(", ")}</p>
                        )}
                        <div className="flex items-center gap-2 text-xs text-muted-foreground mt-0.5">
                          {candidate.start_year && <span>{candidate.start_year}</span>}
                          {candidate.total_volumes != null && <span>{candidate.total_volumes} vol.</span>}
                          {r.provider_used && (
                            <span className="inline-flex items-center gap-1">
                              <ProviderIcon provider={r.provider_used} size={12} />
                              {providerLabel(r.provider_used)}
                            </span>
                          )}
                        </div>
                        {candidate.description && (
                          <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{candidate.description}</p>
                        )}
                      </div>
                      <div className="flex items-start gap-1 shrink-0">
                        <Button
                          size="sm"
                          onClick={(e) => { e.stopPropagation(); handleLink(r); }}
                          disabled={isLinking}
                        >
                          {isLinking ? (
                            <Icon name="spinner" size="sm" className="animate-spin" />
                          ) : (
                            <Icon name="check" size="sm" />
                          )}
                          <span className="ml-1">{t("jobDetail.linkAction")}</span>
                        </Button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Other results (auto_matched, already_linked, no_results, errors) */}
      {others.length > 0 && (
        <div className="border border-border rounded-xl overflow-hidden">
          <div className="px-4 py-3 bg-muted/30 border-b border-border/40">
            <h3 className="font-semibold text-sm text-foreground">{t("jobDetail.resultsBySeries")}</h3>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t("jobDetail.seriesProcessed", { count: String(others.length) })}
            </p>
          </div>
          <div className="max-h-[400px] overflow-y-auto">
            {others.map((r) => (
              <div key={r.id} className="flex items-center gap-2 px-4 py-1.5 border-b border-border/10 last:border-b-0 text-sm">
                {libraryId && r.series_id ? (
                  <Link href={`/series/${r.series_id}`} className="text-primary hover:underline truncate">{r.series_name}</Link>
                ) : (
                  <span className="text-foreground truncate">{r.series_name}</span>
                )}
                <span className={`ml-auto text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap shrink-0 ${
                  r.status === "auto_matched" ? "bg-success/20 text-success" :
                  r.status === "already_linked" ? "bg-primary/20 text-primary" :
                  r.status === "error" ? "bg-destructive/20 text-destructive" :
                  "bg-muted text-muted-foreground"
                }`}>
                  {r.status === "auto_matched" ? t("jobDetail.autoMatched") :
                   r.status === "already_linked" ? t("jobDetail.alreadyLinked") :
                   r.status === "no_results" ? t("jobDetail.noResults") :
                   r.status === "error" ? t("common.error") :
                   r.status}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

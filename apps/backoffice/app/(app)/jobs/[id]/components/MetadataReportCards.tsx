import Link from "next/link";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, StatBox } from "@/app/components/ui";
import type { MetadataBatchReportDto, MetadataBatchResultDto, MetadataRefreshReportDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { SeriesResultRow, ResultStatusBadge } from "./SeriesResultRow";

export function MetadataBatchReportCard({ report, t }: { report: MetadataBatchReportDto; t: TranslateFunction }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.batchReport")}</CardTitle>
        <CardDescription>{t("jobDetail.seriesAnalyzed", { count: String(report.total_series) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <StatBox value={report.auto_matched} label={t("jobDetail.autoMatched")} variant="success" />
          <StatBox value={report.already_linked} label={t("jobDetail.alreadyLinked")} variant="primary" />
          <StatBox value={report.no_results} label={t("jobDetail.noResults")} />
          <StatBox value={report.too_many_results} label={t("jobDetail.tooManyResults")} variant="warning" />
          <StatBox value={report.low_confidence} label={t("jobDetail.lowConfidence")} variant="warning" />
          <StatBox value={report.errors} label={t("jobDetail.errors")} variant={report.errors > 0 ? "error" : "default"} />
        </div>
      </CardContent>
    </Card>
  );
}

export function MetadataBatchResultsCard({ results, libraryId, t }: {
  results: MetadataBatchResultDto[];
  libraryId: string | null;
  t: TranslateFunction;
}) {
  if (results.length === 0) return null;

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.resultsBySeries")}</CardTitle>
        <CardDescription>{t("jobDetail.seriesProcessed", { count: String(results.length) })}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-2 max-h-[600px] overflow-y-auto">
        {results.map((r) => (
          <SeriesResultRow
            key={r.id}
            seriesId={r.series_id}
            seriesName={r.series_name}
            libraryId={libraryId}
            tone={
              r.status === "auto_matched" ? "bg-success/10 border-success/20" :
              r.status === "already_linked" ? "bg-primary/10 border-primary/20" :
              r.status === "error" ? "bg-destructive/10 border-destructive/20" :
              "bg-muted/50 border-border/60"
            }
            badge={
              <ResultStatusBadge className={
                r.status === "auto_matched" ? "bg-success/20 text-success" :
                r.status === "already_linked" ? "bg-primary/20 text-primary" :
                r.status === "no_results" ? "bg-muted text-muted-foreground" :
                r.status === "too_many_results" ? "bg-amber-500/15 text-amber-600" :
                r.status === "low_confidence" ? "bg-amber-500/15 text-amber-600" :
                r.status === "error" ? "bg-destructive/20 text-destructive" :
                "bg-muted text-muted-foreground"
              }>
                {r.status === "auto_matched" ? t("jobDetail.autoMatched") :
                 r.status === "already_linked" ? t("jobDetail.alreadyLinked") :
                 r.status === "no_results" ? t("jobDetail.noResults") :
                 r.status === "too_many_results" ? t("jobDetail.tooManyResults") :
                 r.status === "low_confidence" ? t("jobDetail.lowConfidence") :
                 r.status === "error" ? t("common.error") :
                 r.status}
              </ResultStatusBadge>
            }
            errorMessage={r.error_message}
          >
            <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
              {r.provider_used && (
                <span>{r.provider_used}{r.fallback_used ? ` ${t("metadata.fallbackUsed")}` : ""}</span>
              )}
              {r.candidates_count > 0 && (
                <span>{r.candidates_count} {t("jobDetail.candidates", { plural: r.candidates_count > 1 ? "s" : "" })}</span>
              )}
              {r.best_confidence != null && (
                <span>{Math.round(r.best_confidence * 100)}% {t("jobDetail.confidence")}</span>
              )}
            </div>
            {r.best_candidate_json && (
              <p className="text-xs text-muted-foreground mt-1">
                {t("jobDetail.match", { title: (r.best_candidate_json as { title?: string }).title || r.best_candidate_json.toString() })}
              </p>
            )}
          </SeriesResultRow>
        ))}
      </CardContent>
    </Card>
  );
}

export function MetadataRefreshReportCard({ report, t }: { report: MetadataRefreshReportDto; t: TranslateFunction }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.refreshReport")}</CardTitle>
        <CardDescription>{t("jobDetail.refreshReportDesc", { count: String(report.total_links) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          <StatBox
            value={report.refreshed}
            label={t("jobDetail.refreshed")}
            variant="success"
            icon={
              <svg className="w-6 h-6 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            }
          />
          <StatBox value={report.unchanged} label={t("jobDetail.unchanged")} />
          <StatBox value={report.errors} label={t("jobDetail.errors")} variant={report.errors > 0 ? "error" : "default"} />
          <StatBox value={report.total_links} label={t("jobDetail.total")} />
        </div>
      </CardContent>
    </Card>
  );
}

export function MetadataRefreshChangesCard({ report, libraryId, t }: {
  report: MetadataRefreshReportDto;
  libraryId: string | null;
  t: TranslateFunction;
}) {
  if (report.changes.length === 0) return null;

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.refreshChanges")}</CardTitle>
        <CardDescription>{t("jobDetail.refreshChangesDesc", { count: String(report.changes.length) })}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3 max-h-[600px] overflow-y-auto">
        {report.changes.map((r, idx) => (
          <SeriesResultRow
            key={idx}
            seriesId={r.series_id}
            seriesName={r.series_name}
            libraryId={libraryId}
            tone={
              r.status === "updated" ? "bg-success/10 border-success/20" :
              r.status === "error" ? "bg-destructive/10 border-destructive/20" :
              "bg-muted/50 border-border/60"
            }
            badge={
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-muted-foreground">{r.provider}</span>
                <ResultStatusBadge className={
                  r.status === "updated" ? "bg-success/20 text-success" :
                  r.status === "error" ? "bg-destructive/20 text-destructive" :
                  "bg-muted text-muted-foreground"
                }>
                  {r.status === "updated" ? t("jobDetail.refreshed") :
                   r.status === "error" ? t("common.error") :
                   t("jobDetail.unchanged")}
                </ResultStatusBadge>
              </div>
            }
            errorMessage={r.error}
          >
            {r.series_changes.length > 0 && (
              <div className="mt-2">
                <span className="text-[10px] uppercase tracking-wide text-muted-foreground font-semibold">{t("metadata.seriesLabel")}</span>
                <div className="mt-1 space-y-1">
                  {r.series_changes.map((c, ci) => (
                    <div key={ci} className="flex items-start gap-2 text-xs">
                      <span className="font-medium text-foreground shrink-0 w-24">{t(`field.${c.field}` as never) || c.field}</span>
                      <span className="text-muted-foreground line-through truncate max-w-[200px]" title={String(c.old ?? "—")}>
                        {c.old != null ? (Array.isArray(c.old) ? (c.old as string[]).join(", ") : String(c.old)) : "—"}
                      </span>
                      <span className="text-success shrink-0">→</span>
                      <span className="text-success truncate max-w-[200px]" title={String(c.new ?? "—")}>
                        {c.new != null ? (Array.isArray(c.new) ? (c.new as string[]).join(", ") : String(c.new)) : "—"}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {r.book_changes.length > 0 && (
              <div className="mt-2">
                <span className="text-[10px] uppercase tracking-wide text-muted-foreground font-semibold">
                  {t("metadata.booksLabel")} ({r.book_changes.length})
                </span>
                <div className="mt-1 space-y-2">
                  {r.book_changes.map((b, bi) => (
                    <div key={bi} className="pl-2 border-l-2 border-border/60">
                      <Link
                        href={`/books/${b.book_id}`}
                        className="text-xs text-primary hover:underline font-medium"
                      >
                        {b.volume != null && <span className="text-muted-foreground mr-1">T.{b.volume}</span>}
                        {b.title}
                      </Link>
                      <div className="mt-0.5 space-y-0.5">
                        {b.changes.map((c, ci) => (
                          <div key={ci} className="flex items-start gap-2 text-xs">
                            <span className="font-medium text-foreground shrink-0 w-24">{t(`field.${c.field}` as never) || c.field}</span>
                            <span className="text-muted-foreground line-through truncate max-w-[150px]" title={String(c.old ?? "—")}>
                              {c.old != null ? (Array.isArray(c.old) ? (c.old as string[]).join(", ") : String(c.old).substring(0, 60)) : "—"}
                            </span>
                            <span className="text-success shrink-0">→</span>
                            <span className="text-success truncate max-w-[150px]" title={String(c.new ?? "—")}>
                              {c.new != null ? (Array.isArray(c.new) ? (c.new as string[]).join(", ") : String(c.new).substring(0, 60)) : "—"}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </SeriesResultRow>
        ))}
      </CardContent>
    </Card>
  );
}

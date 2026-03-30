import Link from "next/link";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, StatBox } from "@/app/components/ui";
import type { ReadingStatusMatchReportDto, ReadingStatusMatchResultDto, ReadingStatusPushReportDto, ReadingStatusPushResultDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

export function ReadingStatusMatchReportCard({ report, t }: { report: ReadingStatusMatchReportDto; t: TranslateFunction }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.readingStatusMatchReport")}</CardTitle>
        <CardDescription>{t("jobDetail.seriesAnalyzed", { count: String(report.total_series) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <StatBox value={report.linked} label={t("jobDetail.linked")} variant="success" />
          <StatBox value={report.already_linked} label={t("jobDetail.alreadyLinked")} variant="primary" />
          <StatBox value={report.no_results} label={t("jobDetail.noResults")} />
          <StatBox value={report.ambiguous} label={t("jobDetail.ambiguous")} variant="warning" />
          <StatBox value={report.errors} label={t("jobDetail.errors")} variant={report.errors > 0 ? "error" : "default"} />
        </div>
      </CardContent>
    </Card>
  );
}

export function ReadingStatusMatchResultsCard({ results, libraryId, t }: {
  results: ReadingStatusMatchResultDto[];
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
          <div
            key={r.id}
            className={`p-3 rounded-lg border ${
              r.status === "linked" ? "bg-success/10 border-success/20" :
              r.status === "already_linked" ? "bg-primary/10 border-primary/20" :
              r.status === "error" ? "bg-destructive/10 border-destructive/20" :
              r.status === "ambiguous" ? "bg-amber-500/10 border-amber-500/20" :
              "bg-muted/50 border-border/60"
            }`}
          >
            <div className="flex items-center justify-between gap-2">
              {libraryId ? (
                <Link
                  href={`/series/${r.series_id ?? encodeURIComponent(r.series_name)}`}
                  className="font-medium text-sm text-primary hover:underline truncate"
                >
                  {r.series_name}
                </Link>
              ) : (
                <span className="font-medium text-sm text-foreground truncate">{r.series_name}</span>
              )}
              <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap ${
                r.status === "linked" ? "bg-success/20 text-success" :
                r.status === "already_linked" ? "bg-primary/20 text-primary" :
                r.status === "no_results" ? "bg-muted text-muted-foreground" :
                r.status === "ambiguous" ? "bg-amber-500/15 text-amber-600" :
                r.status === "error" ? "bg-destructive/20 text-destructive" :
                "bg-muted text-muted-foreground"
              }`}>
                {r.status === "linked" ? t("jobDetail.linked") :
                 r.status === "already_linked" ? t("jobDetail.alreadyLinked") :
                 r.status === "no_results" ? t("jobDetail.noResults") :
                 r.status === "ambiguous" ? t("jobDetail.ambiguous") :
                 r.status === "error" ? t("common.error") :
                 r.status}
              </span>
            </div>
            {r.status === "linked" && r.anilist_title && (
              <div className="mt-1 flex items-center gap-1.5 text-xs text-muted-foreground">
                <svg className="w-3 h-3 text-success shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                </svg>
                {r.anilist_url ? (
                  <a href={r.anilist_url} target="_blank" rel="noopener noreferrer" className="text-success hover:underline">
                    {r.anilist_title}
                  </a>
                ) : (
                  <span className="text-success">{r.anilist_title}</span>
                )}
                {r.anilist_id && <span className="text-muted-foreground/60">#{r.anilist_id}</span>}
              </div>
            )}
            {r.error_message && (
              <p className="text-xs text-destructive/80 mt-1">{r.error_message}</p>
            )}
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

export function ReadingStatusPushReportCard({ report, t }: { report: ReadingStatusPushReportDto; t: TranslateFunction }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.readingStatusPushReport")}</CardTitle>
        <CardDescription>{t("jobDetail.seriesAnalyzed", { count: String(report.total_series) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <StatBox value={report.pushed} label={t("jobDetail.pushed")} variant="success" />
          <StatBox value={report.skipped} label={t("jobDetail.skipped")} variant="primary" />
          <StatBox value={report.no_books} label={t("jobDetail.noBooks")} />
          <StatBox value={report.errors} label={t("jobDetail.errors")} variant={report.errors > 0 ? "error" : "default"} />
        </div>
      </CardContent>
    </Card>
  );
}

export function ReadingStatusPushResultsCard({ results, libraryId, t }: {
  results: ReadingStatusPushResultDto[];
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
          <div
            key={r.id}
            className={`p-3 rounded-lg border ${
              r.status === "pushed" ? "bg-success/10 border-success/20" :
              r.status === "error" ? "bg-destructive/10 border-destructive/20" :
              r.status === "skipped" ? "bg-primary/10 border-primary/20" :
              "bg-muted/50 border-border/60"
            }`}
          >
            <div className="flex items-center justify-between gap-2">
              {libraryId ? (
                <Link
                  href={`/series/${r.series_id ?? encodeURIComponent(r.series_name)}`}
                  className="font-medium text-sm text-primary hover:underline truncate"
                >
                  {r.series_name}
                </Link>
              ) : (
                <span className="font-medium text-sm text-foreground truncate">{r.series_name}</span>
              )}
              <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap ${
                r.status === "pushed" ? "bg-success/20 text-success" :
                r.status === "skipped" ? "bg-primary/20 text-primary" :
                r.status === "no_books" ? "bg-muted text-muted-foreground" :
                r.status === "error" ? "bg-destructive/20 text-destructive" :
                "bg-muted text-muted-foreground"
              }`}>
                {r.status === "pushed" ? t("jobDetail.pushed") :
                 r.status === "skipped" ? t("jobDetail.skipped") :
                 r.status === "no_books" ? t("jobDetail.noBooks") :
                 r.status === "error" ? t("common.error") :
                 r.status}
              </span>
            </div>
            {r.status === "pushed" && r.anilist_title && (
              <div className="mt-1 flex items-center gap-1.5 text-xs text-muted-foreground">
                <svg className="w-3 h-3 text-success shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
                {r.anilist_url ? (
                  <a href={r.anilist_url} target="_blank" rel="noopener noreferrer" className="text-success hover:underline">
                    {r.anilist_title}
                  </a>
                ) : (
                  <span className="text-success">{r.anilist_title}</span>
                )}
                {r.anilist_status && <span className="text-muted-foreground/70 font-medium">{r.anilist_status}</span>}
                {r.progress_volumes != null && <span className="text-muted-foreground/60">vol. {r.progress_volumes}</span>}
              </div>
            )}
            {r.error_message && (
              <p className="text-xs text-destructive/80 mt-1">{r.error_message}</p>
            )}
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

import Link from "next/link";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface AllSeries {
  series_name: string;
  book_count: number;
  extracted_names?: string[];
}

interface MatchedSeries {
  telegram_name: string;
  series_id: string;
  series_name: string;
  book_count: number;
}

interface TelegramSyncResultsCardProps {
  new_books: number;
  series_searched: number;
  all_series: AllSeries[];
  matched_series: MatchedSeries[];
  t: TranslateFunction;
}

export function TelegramSyncResultsCard({ new_books, series_searched, all_series, matched_series, t }: TelegramSyncResultsCardProps) {
  const matchedLocalNames = new Set(matched_series.map(m => m.series_name));

  const withMatch = all_series.filter(s => matchedLocalNames.has(s.series_name));
  const withoutMatch = all_series.filter(s => !matchedLocalNames.has(s.series_name));

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.telegramSyncMatched")}</CardTitle>
        <CardDescription>
          {t("jobDetail.telegramSyncMessages").replace("{{new}}", String(new_books)).replace("{{searched}}", String(series_searched))}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">

        {/* Series that had Telegram results — split by match status */}
        <div className="space-y-4">
          <h4 className="text-sm font-semibold text-foreground">
            {t("jobDetail.telegramSyncSeriesFound").replace("{{count}}", String(all_series.length))}
          </h4>

          {/* Matched against local library */}
          {withMatch.length > 0 && (
            <div>
              <p className="text-xs font-medium text-success mb-1.5">{t("jobDetail.telegramWithMatch").replace("{{count}}", String(withMatch.length))}</p>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-6">
                {withMatch.map(s => (
                  <div key={s.series_name} className="flex items-center gap-2 py-1 border-b border-border/30 last:border-0">
                    <span className="text-sm truncate flex-1">{s.series_name}</span>
                    <span className="text-xs text-muted-foreground shrink-0 tabular-nums">{s.book_count}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* No local match — show why */}
          {withoutMatch.length > 0 && (
            <div>
              <p className="text-xs font-medium text-warning mb-1.5">{t("jobDetail.telegramWithoutMatch").replace("{{count}}", String(withoutMatch.length))}</p>
              <div className="space-y-2">
                {withoutMatch.map(s => {
                  const queryLower = s.series_name.toLowerCase();
                  const relevantNames = s.extracted_names
                    ? s.extracted_names.filter(n => n.toLowerCase().startsWith(queryLower) && n !== s.series_name)
                    : [];
                  return (
                    <div key={s.series_name} className="py-1.5 border-b border-border/30 last:border-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm flex-1">{s.series_name}</span>
                        <span className="text-xs text-muted-foreground shrink-0 tabular-nums">{s.book_count}</span>
                      </div>
                      {relevantNames.length > 0 && (
                        <p className="text-xs text-muted-foreground mt-0.5 truncate" title={relevantNames.join(", ")}>
                          {t("jobDetail.telegramExtractedNames")} {relevantNames.join(", ")}
                        </p>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {all_series.length === 0 && (
            <p className="text-sm text-muted-foreground">{t("jobDetail.telegramSyncSeriesNone")}</p>
          )}
        </div>

        {/* Matched series with links to local series */}
        <div>
          <h4 className="text-sm font-semibold text-foreground mb-2">
            {t("jobDetail.telegramSyncSeriesMatched").replace("{{count}}", String(matched_series.length))}
          </h4>
          {matched_series.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("jobDetail.telegramSyncMatchedNone")}</p>
          ) : (
            <div className="space-y-1">
              {matched_series.map((m) => (
                <div key={`${m.series_id}-${m.telegram_name}`} className="flex items-center gap-3 py-1.5 border-b border-border/30 last:border-0">
                  <span className="text-sm text-muted-foreground truncate w-1/2">{m.telegram_name}</span>
                  <svg className="w-3 h-3 text-muted-foreground shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                  <Link
                    href={`/series/${m.series_id}`}
                    className="text-sm font-medium text-primary hover:underline truncate flex-1"
                  >
                    {m.series_name}
                  </Link>
                  <span className="text-xs text-muted-foreground shrink-0 tabular-nums">{m.book_count}</span>
                </div>
              ))}
            </div>
          )}
        </div>

      </CardContent>
    </Card>
  );
}

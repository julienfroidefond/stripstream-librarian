import { Card, CardHeader, CardTitle, CardDescription, CardContent, StatBox } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface SourceResult {
  username: string;
  new_books: number;
}

interface RecentBook {
  filename: string;
  series_name: string | null;
  volume_number: number | null;
  channel: string;
}

interface TelegramIncrementalResultsCardProps {
  new_books: number;
  sources_scanned: number;
  sources: SourceResult[];
  recent_books: RecentBook[];
  t: TranslateFunction;
}

export function TelegramIncrementalResultsCard({ new_books, sources_scanned, sources, recent_books, t }: TelegramIncrementalResultsCardProps) {
  const activeSources = sources.filter(s => s.new_books > 0);

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.telegramIncrementalTitle")}</CardTitle>
        <CardDescription>
          {t("jobDetail.telegramIncrementalDesc")
            .replace("{{new}}", String(new_books))
            .replace("{{sources}}", String(sources_scanned))}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">

        {/* Summary stats */}
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <StatBox
            label={t("jobDetail.telegramIncrementalNewBooks")}
            value={String(new_books)}
            variant={new_books > 0 ? "success" : "default"}
          />
          <StatBox
            label={t("jobDetail.telegramIncrementalSources")}
            value={String(sources_scanned)}
            variant="primary"
          />
        </div>

        {/* Per-source breakdown */}
        {sources.length > 0 && (
          <div>
            <h4 className="text-sm font-semibold text-foreground mb-3">
              {t("jobDetail.telegramIncrementalPerSource")}
            </h4>
            <div className="space-y-1">
              {sources.map(s => (
                <div key={s.username} className="flex items-center justify-between py-1.5 border-b border-border/30 last:border-0">
                  <div className="flex items-center gap-2">
                    <svg className="w-3.5 h-3.5 text-muted-foreground shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
                    </svg>
                    <span className="text-sm font-mono text-foreground">@{s.username}</span>
                  </div>
                  <span className={`text-sm font-medium tabular-nums ${s.new_books > 0 ? "text-success" : "text-muted-foreground"}`}>
                    {s.new_books > 0 ? `+${s.new_books}` : "—"}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* New books list */}
        {recent_books.length > 0 ? (
          <div>
            <h4 className="text-sm font-semibold text-foreground mb-3">
              {t("jobDetail.telegramIncrementalRecentBooks").replace("{{count}}", String(recent_books.length))}
            </h4>
            <div className="space-y-1 max-h-96 overflow-y-auto">
              {recent_books.map((b, i) => (
                <div key={i} className="flex items-start gap-3 py-1.5 border-b border-border/30 last:border-0">
                  <div className="flex-1 min-w-0">
                    <p className="text-sm truncate" title={b.filename}>{b.filename}</p>
                    {b.series_name && (
                      <p className="text-xs text-muted-foreground truncate">
                        {b.series_name}{b.volume_number != null ? ` — T${b.volume_number}` : ""}
                      </p>
                    )}
                  </div>
                  <span className="text-xs text-muted-foreground shrink-0 font-mono">@{b.channel}</span>
                </div>
              ))}
            </div>
          </div>
        ) : (
          activeSources.length === 0 && sources_scanned > 0 && (
            <p className="text-sm text-muted-foreground">{t("jobDetail.telegramIncrementalNone")}</p>
          )
        )}

      </CardContent>
    </Card>
  );
}

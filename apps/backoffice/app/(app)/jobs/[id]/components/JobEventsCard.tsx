import Link from "next/link";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

export interface JobEvent {
  id: string;
  job_id: string;
  event_type: string;
  level: string;
  entity_type: string | null;
  entity_id: string | null;
  entity_name: string | null;
  message: string | null;
  detail: Record<string, unknown> | null;
  created_at: string;
}

function LevelIcon({ level }: { level: string }) {
  if (level === "error") {
    return (
      <svg className="w-4 h-4 text-destructive shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    );
  }
  if (level === "warning") {
    return (
      <svg className="w-4 h-4 text-amber-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.732 16.5c-.77.833.192 2.5 1.732 2.5z" />
      </svg>
    );
  }
  return (
    <svg className="w-4 h-4 text-muted-foreground shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

function EventTypeBadge({ eventType }: { eventType: string }) {
  return (
    <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-muted text-muted-foreground shrink-0">
      {eventType}
    </span>
  );
}

function EntityLink({ event }: { event: JobEvent }) {
  if (!event.entity_name) return null;

  if (event.entity_id && event.entity_type === "book") {
    return (
      <Link href={`/books/${event.entity_id}`} className="text-primary hover:underline text-xs truncate">
        {event.entity_name}
      </Link>
    );
  }

  if (event.entity_id && event.entity_type === "series") {
    return (
      <Link href={`/series/${event.entity_id}`} className="text-primary hover:underline text-xs truncate">
        {event.entity_name}
      </Link>
    );
  }

  return <span className="text-xs text-foreground truncate">{event.entity_name}</span>;
}

function levelRowClass(level: string): string {
  if (level === "error") return "bg-destructive/5 border-l-2 border-destructive/40";
  if (level === "warning") return "bg-amber-500/5 border-l-2 border-amber-500/40";
  return "";
}

interface JobEventsCardProps {
  events: JobEvent[];
  t: TranslateFunction;
  locale: string;
}

export function JobEventsCard({ events, t, locale }: JobEventsCardProps) {
  if (events.length === 0) return null;

  const countByLevel = {
    info: events.filter((e) => e.level === "info").length,
    warning: events.filter((e) => e.level === "warning").length,
    error: events.filter((e) => e.level === "error").length,
  };

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.events")}</CardTitle>
        <CardDescription>{t("jobDetail.eventsDesc", { count: String(events.length) })}</CardDescription>
      </CardHeader>
      <CardContent>
        {/* Summary bar */}
        <div className="flex gap-3 mb-4 text-xs">
          <span className="flex items-center gap-1 text-muted-foreground">
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            {countByLevel.info} info
          </span>
          {countByLevel.warning > 0 && (
            <span className="flex items-center gap-1 text-amber-500">
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.732 16.5c-.77.833.192 2.5 1.732 2.5z" />
              </svg>
              {countByLevel.warning} {t("jobDetail.warnings").toLowerCase()}
            </span>
          )}
          {countByLevel.error > 0 && (
            <span className="flex items-center gap-1 text-destructive">
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              {countByLevel.error} {t("jobDetail.errors").toLowerCase()}
            </span>
          )}
        </div>

        {/* Events list */}
        <div className="max-h-[500px] overflow-y-auto space-y-1">
          {events.map((event) => (
            <div
              key={event.id}
              className={`flex flex-wrap items-start gap-x-2 gap-y-0.5 px-2 py-1.5 rounded text-xs ${levelRowClass(event.level)}`}
            >
              <div className="flex items-start gap-2 min-w-0 flex-1">
                <LevelIcon level={event.level} />
                <EventTypeBadge eventType={event.event_type} />
                <EntityLink event={event} />
                {event.message && (
                  <span className={`truncate ${
                    event.level === "error" ? "text-destructive" :
                    event.level === "warning" ? "text-amber-600 dark:text-amber-400" :
                    "text-muted-foreground"
                  }`}>
                    {event.message}
                  </span>
                )}
              </div>
              <span className="text-[10px] text-muted-foreground shrink-0 ml-auto">
                {new Date(event.created_at).toLocaleString(locale)}
              </span>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

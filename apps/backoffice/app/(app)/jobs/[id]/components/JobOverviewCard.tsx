import Link from "next/link";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, StatusBadge, JobTypeBadge } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface JobOverviewCardProps {
  job: {
    id: string;
    type: string;
    status: string;
    library_id: string | null;
    book_id: string | null;
    started_at: string | null;
    finished_at: string | null;
  };
  typeInfo: { label: string; description: string | null };
  t: TranslateFunction;
  formatDuration: (start: string, end: string | null) => string;
}

export function JobOverviewCard({ job, typeInfo, t, formatDuration }: JobOverviewCardProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.overview")}</CardTitle>
        {typeInfo.description && (
          <CardDescription>{typeInfo.description}</CardDescription>
        )}
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex items-center justify-between py-2 border-b border-border/60">
          <span className="text-sm text-muted-foreground">ID</span>
          <code className="px-2 py-1 bg-muted rounded font-mono text-sm text-foreground">{job.id}</code>
        </div>
        <div className="flex items-center justify-between py-2 border-b border-border/60">
          <span className="text-sm text-muted-foreground">{t("jobsList.type")}</span>
          <div className="flex items-center gap-2">
            <JobTypeBadge type={job.type} />
            <span className="text-sm text-muted-foreground">{typeInfo.label}</span>
          </div>
        </div>
        <div className="flex items-center justify-between py-2 border-b border-border/60">
          <span className="text-sm text-muted-foreground">{t("jobsList.status")}</span>
          <StatusBadge status={job.status} />
        </div>
        <div className={`flex items-center justify-between py-2 ${(job.book_id || job.started_at) ? "border-b border-border/60" : ""}`}>
          <span className="text-sm text-muted-foreground">{t("jobDetail.library")}</span>
          <span className="text-sm text-foreground">{job.library_id || t("jobDetail.allLibraries")}</span>
        </div>
        {job.book_id && (
          <div className={`flex items-center justify-between py-2 ${job.started_at ? "border-b border-border/60" : ""}`}>
            <span className="text-sm text-muted-foreground">{t("jobDetail.book")}</span>
            <Link
              href={`/books/${job.book_id}`}
              className="text-sm text-primary hover:text-primary/80 font-mono hover:underline"
            >
              {job.book_id.slice(0, 8)}…
            </Link>
          </div>
        )}
        {job.started_at && (
          <div className="flex items-center justify-between py-2">
            <span className="text-sm text-muted-foreground">{t("jobsList.duration")}</span>
            <span className="text-sm font-semibold text-foreground">
              {formatDuration(job.started_at, job.finished_at)}
            </span>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

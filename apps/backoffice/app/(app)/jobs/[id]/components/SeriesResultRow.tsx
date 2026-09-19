import { ReactNode } from "react";
import Link from "next/link";

interface SeriesResultRowProps {
  seriesId: string | null | undefined;
  seriesName: string;
  libraryId: string | null;
  tone: string;
  badge: ReactNode;
  errorMessage?: string | null;
  children?: ReactNode;
}

export function SeriesResultRow({ seriesId, seriesName, libraryId, tone, badge, errorMessage, children }: SeriesResultRowProps) {
  return (
    <div className={`p-3 rounded-lg border ${tone}`}>
      <div className="flex items-center justify-between gap-2">
        {libraryId && seriesId ? (
          <Link
            href={`/series/${seriesId}`}
            className="font-medium text-sm text-primary hover:underline truncate"
          >
            {seriesName}
          </Link>
        ) : (
          <span className="font-medium text-sm text-foreground truncate">{seriesName}</span>
        )}
        {badge}
      </div>
      {children}
      {errorMessage && <p className="text-xs text-destructive/80 mt-1">{errorMessage}</p>}
    </div>
  );
}

export function ResultStatusBadge({ className, children }: { className: string; children: ReactNode }) {
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap ${className}`}>
      {children}
    </span>
  );
}

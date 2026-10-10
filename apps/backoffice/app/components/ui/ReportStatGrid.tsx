import type { ComponentProps } from "react";

import { StatBox } from "./StatBox";

export type ReportStat = ComponentProps<typeof StatBox>;

interface ReportStatGridProps {
  stats: ReportStat[];
  /** Tailwind `grid-cols-*` classes controlling the column count. */
  className?: string;
}

/** Responsive grid of `StatBox` tiles used by the job report cards. */
export function ReportStatGrid({ stats, className = "grid-cols-2 sm:grid-cols-3" }: ReportStatGridProps) {
  return (
    <div className={`grid ${className} gap-4`}>
      {stats.map((stat, i) => (
        <StatBox key={i} {...stat} />
      ))}
    </div>
  );
}

"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useCallback } from "react";

interface SeriesFiltersProps {
  basePath: string;
  currentSeriesStatus?: string;
  currentHasMissing: boolean;
  seriesStatusOptions: { value: string; label: string }[];
}

export function SeriesFilters({ basePath, currentSeriesStatus, currentHasMissing, seriesStatusOptions }: SeriesFiltersProps) {
  const router = useRouter();
  const searchParams = useSearchParams();

  const updateFilter = useCallback((key: string, value: string) => {
    const params = new URLSearchParams(searchParams.toString());
    if (value) {
      params.set(key, value);
    } else {
      params.delete(key);
    }
    params.delete("page");
    const qs = params.toString();
    router.push(`${basePath}${qs ? `?${qs}` : ""}` as any);
  }, [router, searchParams, basePath]);

  return (
    <div className="flex flex-wrap gap-3">
      <select
        value={currentSeriesStatus || ""}
        onChange={(e) => updateFilter("series_status", e.target.value)}
        className="px-3 py-2 rounded-lg border border-border bg-card text-foreground text-sm"
      >
        {seriesStatusOptions.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>

      <select
        value={currentHasMissing ? "true" : ""}
        onChange={(e) => updateFilter("has_missing", e.target.value)}
        className="px-3 py-2 rounded-lg border border-border bg-card text-foreground text-sm"
      >
        <option value="">Tous</option>
        <option value="true">Livres manquants</option>
      </select>
    </div>
  );
}

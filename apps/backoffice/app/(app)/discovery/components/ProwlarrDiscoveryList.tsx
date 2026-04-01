"use client";

import { useState, useEffect } from "react";
import { Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

interface ProwlarrItem {
  series_name: string;
  release_count: number;
  best_seeders: number;
  total_seeders: number;
  categories: string[];
  indexers: string[];
  best_release_title: string;
  best_download_url: string | null;
  best_size: number;
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function ProwlarrDiscoveryList() {
  const { t } = useTranslation();
  const [items, setItems] = useState<ProwlarrItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchData() {
      setLoading(true);
      setError(null);
      try {
        const resp = await fetch("/api/discovery/prowlarr?limit=100");
        if (!resp.ok) {
          const data = await resp.json().catch(() => ({}));
          setError(data?.error || `Error ${resp.status}`);
          return;
        }
        const data = await resp.json();
        setItems(data);
      } catch {
        setError("Network error");
      } finally {
        setLoading(false);
      }
    }
    fetchData();
  }, []);

  if (loading) {
    return (
      <div className="flex justify-center py-12">
        <Icon name="spinner" size="lg" className="animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-8 text-muted-foreground text-sm">
        {error}
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="text-center py-12 text-muted-foreground">
        {t("discovery.noResults")}
      </div>
    );
  }

  return (
    <div className="border border-border rounded-xl overflow-hidden">
      <table className="w-full text-sm">
        <thead className="bg-muted/50">
          <tr>
            <th className="text-left px-4 py-2.5 font-medium text-muted-foreground">#</th>
            <th className="text-left px-4 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrSeries")}</th>
            <th className="text-left px-4 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrCategories")}</th>
            <th className="text-right px-4 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrReleases")}</th>
            <th className="text-right px-4 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrSeeders")}</th>
            <th className="text-right px-4 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrSize")}</th>
            <th className="text-left px-4 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrBestRelease")}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {items.map((item, idx) => (
            <tr key={item.series_name} className="hover:bg-muted/30 transition-colors">
              <td className="px-4 py-2 text-muted-foreground text-xs">{idx + 1}</td>
              <td className="px-4 py-2 font-medium text-foreground">{item.series_name}</td>
              <td className="px-4 py-2">
                <div className="flex flex-wrap gap-1">
                  {item.categories.slice(0, 3).map((cat) => (
                    <span key={cat} className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
                      {cat}
                    </span>
                  ))}
                </div>
              </td>
              <td className="px-4 py-2 text-right text-muted-foreground">{item.release_count}</td>
              <td className="px-4 py-2 text-right">
                <span className={`font-medium ${
                  item.best_seeders >= 10 ? "text-green-600" :
                  item.best_seeders >= 3 ? "text-amber-600" :
                  "text-red-500"
                }`}>
                  {item.best_seeders}
                </span>
              </td>
              <td className="px-4 py-2 text-right text-muted-foreground text-xs">{formatSize(item.best_size)}</td>
              <td className="px-4 py-2 text-xs text-muted-foreground max-w-xs truncate" title={item.best_release_title}>
                {item.best_release_title}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

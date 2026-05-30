"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { Button, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { SeriesAddModal, type ProwlarrItem } from "./SeriesAddModal";


const PAGE_SIZE = 25;

interface Library {
  id: string;
  name: string;
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatPublishDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffDays = Math.floor(diffMs / 86400000);
  if (diffDays < 0) return d.toLocaleDateString();
  if (diffDays < 1) return "auj.";
  if (diffDays < 7) return `${diffDays}j`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}sem.`;
  if (diffDays < 365) return `${Math.floor(diffDays / 30)}mois`;
  return d.toLocaleDateString();
}

function categoryShortId(cat: string): string {
  const m = cat.match(/\((\d+)\)$/);
  return m ? m[1] : cat;
}

function formatVolumes(volumes: number[]): string {
  if (volumes.length === 0) return "—";
  if (volumes.length <= 3) return volumes.join(", ");
  const min = volumes[0];
  const max = volumes[volumes.length - 1];
  return `${min}-${max} (${volumes.length})`;
}

export function ProwlarrDiscoveryList({ libraries, nocache = false }: { libraries: Library[]; nocache?: boolean }) {
  const { t } = useTranslation();
  const [items, setItems] = useState<ProwlarrItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [addedSet, setAddedSet] = useState<Set<string>>(new Set());
  const [filterCategory, setFilterCategory] = useState<string>("all");
  const [filterIndexer, setFilterIndexer] = useState<string>("all");
  const [allIndexers, setAllIndexers] = useState<string[]>([]);
  const [filterSearch, setFilterSearch] = useState("");
  const [sort, setSort] = useState<"seeders" | "date">("seeders");
  const [hideMine, setHideMine] = useState(false);
  const [page, setPage] = useState(1);

  // Reset to page 1 whenever filters or sort change
  useEffect(() => {
    setPage(1);
  }, [filterCategory, filterIndexer, filterSearch, sort, hideMine]);


  useEffect(() => {
    async function fetchData() {
      setLoading(true);
      setError(null);
      try {
        const p = new URLSearchParams();
        p.set("sort", sort);
        if (nocache) p.set("nocache", "true");
        if (filterIndexer !== "all") p.set("indexer", filterIndexer);
        if (filterCategory !== "all") p.set("category", categoryShortId(filterCategory));
        const resp = await fetch(`/api/discovery/prowlarr?${p.toString()}`);
        if (!resp.ok) {
          const data = await resp.json().catch(() => ({}));
          setError(data?.error || `Error ${resp.status}`);
          return;
        }
        const data = await resp.json();
        setItems(data.items ?? []);
        if (data.all_indexers?.length) setAllIndexers(data.all_indexers);
      } catch {
        setError("Network error");
      } finally {
        setLoading(false);
      }
    }
    fetchData();
  }, [sort, nocache, filterIndexer, filterCategory]);

  function handleAdded(seriesName: string) {
    setAddedSet((prev) => new Set(prev).add(seriesName));
  }

  if (loading) {
    return (
      <div className="flex justify-center py-12">
        <Icon name="spinner" size="lg" className="animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-8 text-muted-foreground text-sm">{error}</div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="text-center py-12 text-muted-foreground">{t("discovery.noResults")}</div>
    );
  }

  const allCategories = [...new Set(items.flatMap((i) => i.categories))].sort();

  const filteredItems = items
    .filter((i) => !addedSet.has(i.series_name))
    .filter((i) => !hideMine || !i.local_series_id)
    .filter((i) => !filterSearch || i.series_name.toLowerCase().includes(filterSearch.toLowerCase()));

  const totalPages = Math.max(1, Math.ceil(filteredItems.length / PAGE_SIZE));
  const currentPage = Math.min(page, totalPages);
  const pageStart = (currentPage - 1) * PAGE_SIZE;
  const visibleItems = filteredItems.slice(pageStart, pageStart + PAGE_SIZE);

  return (
    <div className="space-y-3">
      {/* Provider (primary filter — upstream) */}
      {allIndexers.length > 0 && (
        <div className="flex flex-wrap gap-1">
          <button
            onClick={() => setFilterIndexer("all")}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition-colors ${
              filterIndexer === "all"
                ? "bg-primary/15 text-primary border-primary/30"
                : "bg-card text-muted-foreground border-border hover:border-primary/30"
            }`}
          >
            {t("discovery.prowlarrAllProviders")}
          </button>
          {allIndexers.map((indexer) => {
            const isActive = filterIndexer === indexer;
            return (
              <button
                key={indexer}
                onClick={() => setFilterIndexer(isActive ? "all" : indexer)}
                className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition-colors ${
                  isActive
                    ? "bg-primary/15 text-primary border-primary/30"
                    : "bg-card text-muted-foreground border-border hover:border-primary/30"
                }`}
              >
                {indexer}
              </button>
            );
          })}
        </div>
      )}
      {/* Secondary filters */}
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="text"
          value={filterSearch}
          onChange={(e) => setFilterSearch(e.target.value)}
          placeholder={t("common.search")}
          className="text-sm border border-border rounded-lg px-3 py-1.5 bg-background w-48"
        />
        <button
          onClick={() => setHideMine((v) => !v)}
          className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition-colors ${
            hideMine
              ? "bg-green-500/15 text-green-600 border-green-500/30"
              : "bg-card text-muted-foreground border-border hover:border-primary/30"
          }`}
        >
          {t("discovery.prowlarrHideMine")}
        </button>
        <div className="flex gap-0.5">
          {(["seeders", "date"] as const).map((s) => (
            <button
              key={s}
              onClick={() => setSort(s)}
              className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition-colors ${
                sort === s
                  ? "bg-primary/15 text-primary border-primary/30"
                  : "bg-card text-muted-foreground border-border hover:border-primary/30"
              }`}
            >
              {s === "seeders" ? t("discovery.prowlarrSortSeeders") : t("discovery.prowlarrSortDate")}
            </button>
          ))}
        </div>
        <div className="flex flex-wrap gap-1">
          <button
            onClick={() => setFilterCategory("all")}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition-colors ${
              filterCategory === "all"
                ? "bg-primary/15 text-primary border-primary/30"
                : "bg-card text-muted-foreground border-border hover:border-primary/30"
            }`}
          >
            {t("common.all")} ({items.filter((i) => !addedSet.has(i.series_name)).length})
          </button>
          {allCategories.map((cat) => {
            const isActive = filterCategory === cat;
            return (
              <button
                key={cat}
                onClick={() => setFilterCategory(isActive ? "all" : cat)}
                className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition-colors ${
                  isActive
                    ? "bg-primary/15 text-primary border-primary/30"
                    : "bg-card text-muted-foreground border-border hover:border-primary/30"
                }`}
              >
                {cat} · {items.filter((i) => !addedSet.has(i.series_name) && i.categories.includes(cat)).length}
              </button>
            );
          })}
        </div>
      </div>

    <div className="border border-border rounded-xl overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-muted/50">
            <tr>
              <th className="text-left px-3 py-2.5 font-medium text-muted-foreground w-8">#</th>
              <th className="text-left px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrSeries")}</th>
              <th className="text-left px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrProvider")}</th>
              <th className="text-left px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrCategories")}</th>
              <th className="text-center px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.volumes", { count: "" })}</th>
              <th className="text-right px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrReleases")}</th>
              <th className="text-right px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrSeeders")}</th>
              <th className="text-right px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrSize")}</th>
              <th className="text-right px-3 py-2.5 font-medium text-muted-foreground">{t("discovery.prowlarrDate")}</th>
              <th className="text-right px-3 py-2.5 font-medium text-muted-foreground w-24"></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {visibleItems.map((item, idx) => (
              <ProwlarrRow key={item.series_name} item={item} idx={pageStart + idx} libraries={libraries} onAdded={() => handleAdded(item.series_name)} />
            ))}
          </tbody>
        </table>
      </div>
    </div>

    {totalPages > 1 && (
      <div className="flex items-center justify-between gap-2 text-xs">
        <span className="text-muted-foreground">
          {pageStart + 1}–{Math.min(pageStart + PAGE_SIZE, filteredItems.length)} / {filteredItems.length}
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setPage(1)}
            disabled={currentPage === 1}
            className="px-2 py-1 rounded-md border border-border bg-card text-muted-foreground hover:border-primary/30 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            «
          </button>
          <button
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={currentPage === 1}
            className="px-2 py-1 rounded-md border border-border bg-card text-muted-foreground hover:border-primary/30 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            ‹
          </button>
          <span className="px-2 text-muted-foreground tabular-nums">{currentPage} / {totalPages}</span>
          <button
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={currentPage === totalPages}
            className="px-2 py-1 rounded-md border border-border bg-card text-muted-foreground hover:border-primary/30 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            ›
          </button>
          <button
            onClick={() => setPage(totalPages)}
            disabled={currentPage === totalPages}
            className="px-2 py-1 rounded-md border border-border bg-card text-muted-foreground hover:border-primary/30 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            »
          </button>
        </div>
      </div>
    )}
    </div>
  );
}

function ProwlarrRow({ item, idx, libraries, onAdded }: {
  item: ProwlarrItem;
  idx: number;
  libraries: Library[];
  onAdded: () => void;
}) {
  return (
    <tr className="hover:bg-muted/30 transition-colors">
      <td className="px-3 py-2 text-muted-foreground text-xs">{idx + 1}</td>
      <td className="px-3 py-2">
        <div className="flex items-center gap-2">
          <div className="w-8 h-10 rounded bg-muted/50 flex items-center justify-center shrink-0">
            <Icon name="books" size="sm" className="text-muted-foreground/40" />
          </div>
          <div>
            {item.best_info_url ? (
              <a
                href={item.best_info_url}
                target="_blank"
                rel="noopener noreferrer"
                className="font-medium text-primary hover:underline inline-flex items-center gap-1"
              >
                {item.series_name}
                <Icon name="externalLink" size="sm" className="opacity-60" />
              </a>
            ) : (
              <p className="font-medium text-foreground">{item.series_name}</p>
            )}
            <p className="text-[10px] text-muted-foreground truncate max-w-xs" title={item.best_release_title}>
              {item.best_release_title}
            </p>
            {item.local_series_id && (
              <div className="flex flex-wrap items-center gap-1 mt-0.5">
                <Link
                  href={`/series/${item.local_series_id}`}
                  className="text-[10px] text-green-600 dark:text-green-500 hover:underline"
                >
                  ✓ {item.local_series_name ?? item.series_name}
                </Link>
                {item.volumes_already_owned.length > 0 && (
                  <span className="text-[10px] text-muted-foreground">
                    · {item.volumes_already_owned.map((v) => `T${v}`).join(", ")} possédé{item.volumes_already_owned.length > 1 ? "s" : ""}
                  </span>
                )}
              </div>
            )}
          </div>
        </div>
      </td>
      <td className="px-3 py-2 text-xs text-muted-foreground">
        {item.best_indexer ?? "—"}
      </td>
      <td className="px-3 py-2">
        <div className="flex flex-wrap gap-1">
          {item.categories.slice(0, 2).map((cat) => (
            <span key={cat} title={cat} className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground whitespace-nowrap">{categoryShortId(cat)}</span>
          ))}
        </div>
      </td>
      <td className="px-3 py-2 text-center text-xs text-muted-foreground">{formatVolumes(item.volumes_found ?? [])}</td>
      <td className="px-3 py-2 text-right text-muted-foreground">{item.release_count}</td>
      <td className="px-3 py-2 text-right">
        <span className={`font-medium ${item.best_seeders >= 10 ? "text-green-600" : item.best_seeders >= 3 ? "text-amber-600" : "text-red-500"}`}>
          {item.best_seeders}
        </span>
      </td>
      <td className="px-3 py-2 text-right text-muted-foreground text-xs">{formatSize(item.best_size)}</td>
      <td className="px-3 py-2 text-right text-muted-foreground text-xs">{formatPublishDate(item.best_publish_date)}</td>
      <td className="px-3 py-2 text-right">
        <SeriesAddModal mode="prowlarr" item={item} libraries={libraries} onAdded={onAdded}>
          {(open) => (
            <Button variant="outline" size="xs" onClick={open}>+</Button>
          )}
        </SeriesAddModal>
      </td>
    </tr>
  );
}

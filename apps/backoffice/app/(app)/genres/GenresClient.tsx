"use client";

import { useState, useCallback, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import Image from "next/image";
import Link from "next/link";
import { getBookCoverUrl } from "@/lib/api";
import type { SeriesDto, LibraryDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import { LibraryMultiBadgeSelector } from "../jobs/components/LibraryBadgeSelector";

export type GenreDto = {
  name: string;
  series_count: number;
};

type Props = {
  initialGenres: GenreDto[];
  initialUntagged: SeriesDto[];
  libraries: LibraryDto[];
  initialTotalSeries: number;
};

// null = "sans genre", [] = aucun filtre de genre
type GenreFilter = string[] | null;
type SeriesView = "cards" | "table";
type GenreFilterMode = "include" | "exclude";

function mergeGenreCounts(groups: GenreDto[][]): GenreDto[] {
  const counts = new Map<string, number>();
  for (const group of groups) {
    for (const genre of group) counts.set(genre.name, (counts.get(genre.name) ?? 0) + genre.series_count);
  }
  return Array.from(counts, ([name, series_count]) => ({ name, series_count }));
}

function SeriesCoverImage({ series }: { series: SeriesDto }) {
  if (series.first_book_id) {
    return (
      <Image
        src={getBookCoverUrl(series.first_book_id, series.first_book_updated_at)}
        alt={series.name}
        fill
        className="object-cover"
        sizes="80px"
      />
    );
  }
  if (series.cover_url) {
    return (
      /* eslint-disable-next-line @next/next/no-img-element */
      <img src={series.cover_url} alt={series.name} className="w-full h-full object-cover" />
    );
  }
  return null;
}

function GenreSeriesModal({ genre, onClose }: { genre: string; onClose: () => void }) {
  const { t } = useTranslation();
  const [series, setSeries] = useState<SeriesDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);

  useEffect(() => {
    setLoading(true);
    fetch(`/api/series?genre=${encodeURIComponent(genre)}&limit=200`)
      .then(r => r.json())
      .then(data => { setSeries(data.items ?? []); setTotal(data.total ?? 0); })
      .catch(() => setSeries([]))
      .finally(() => setLoading(false));
  }, [genre]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return createPortal(
    <>
      <div className="fixed inset-0 z-40 bg-black/50 backdrop-blur-sm" onClick={onClose} />
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
        <div className="pointer-events-auto w-full max-w-lg max-h-[80vh] bg-background rounded-2xl border border-border shadow-2xl flex flex-col">
          <div className="flex items-center justify-between px-5 py-4 border-b border-border shrink-0">
            <div className="flex items-center gap-2 min-w-0">
              <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-sm font-semibold bg-success/10 text-success border border-success/20 truncate">
                {genre}
              </span>
              {!loading && (
                <span className="text-sm text-muted-foreground shrink-0">
                  {t("genres.seriesCount", { count: String(total), plural: total !== 1 ? "s" : "" })}
                </span>
              )}
            </div>
            <button onClick={onClose} className="ml-3 shrink-0 p-1.5 rounded-lg hover:bg-muted transition-colors text-muted-foreground hover:text-foreground">
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
          <div className="flex-1 overflow-y-auto p-5">
            {loading ? (
              <div className="flex items-center justify-center py-16 text-muted-foreground text-sm">{t("common.loading")}</div>
            ) : series.length === 0 ? (
              <div className="flex items-center justify-center py-16 text-muted-foreground text-sm">{t("genres.noGenres")}</div>
            ) : (
              <div className="space-y-1">
                {series.map(s => (
                  <Link key={s.series_id} href={`/series/${s.series_id}`} onClick={onClose}
                    className="flex items-center gap-3 p-2 rounded-xl hover:bg-muted/60 transition-colors group">
                    <div className="w-10 h-14 relative rounded-lg overflow-hidden bg-muted shrink-0">
                      <SeriesCoverImage series={s} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="font-medium text-sm truncate group-hover:text-primary transition-colors">{s.name}</p>
                      <p className="text-xs text-muted-foreground">{s.book_count} {t("dashboard.books").toLowerCase()}</p>
                    </div>
                    <svg className="w-4 h-4 text-muted-foreground shrink-0 opacity-0 group-hover:opacity-100 transition-opacity" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  </Link>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </>,
    document.body
  );
}

function StatCard({
  emoji,
  label,
  value,
  sub,
  color,
}: {
  emoji: string;
  label: string;
  value: string | number;
  sub?: string;
  color: "violet" | "green" | "blue" | "amber";
}) {
  const palette = {
    violet: "border-violet-500/30 bg-violet-500/5 [&_span]:text-violet-400",
    green:  "border-emerald-500/30 bg-emerald-500/5 [&_span]:text-emerald-400",
    blue:   "border-blue-500/30 bg-blue-500/5 [&_span]:text-blue-400",
    amber:  "border-amber-500/30 bg-amber-500/5 [&_span]:text-amber-400",
  }[color];
  return (
    <div className={`rounded-2xl border p-4 flex flex-col gap-2 ${palette}`}>
      <div className="flex items-center justify-between">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">{label}</p>
        <span className="text-xl">{emoji}</span>
      </div>
      <p className="text-3xl font-bold tracking-tight text-foreground">{value}</p>
      {sub && <p className="text-xs text-muted-foreground">{sub}</p>}
    </div>
  );
}

export function GenresClient({ initialGenres, initialUntagged, libraries, initialTotalSeries }: Props) {
  const { t } = useTranslation();
  const [genres, setGenres] = useState<GenreDto[]>(initialGenres);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [activeGenreModal, setActiveGenreModal] = useState<string | null>(null);
  const [genreFilter, setGenreFilter] = useState("");

  // Genre covers for cards
  const [genreCovers, setGenreCovers] = useState<Record<string, SeriesDto[]>>({});

  // Stats
  const [totalSeries] = useState(initialTotalSeries);

  // Fetch preview covers for all genres once on mount
  useEffect(() => {
    if (initialGenres.length === 0) return;
    Promise.all(
      initialGenres.map(g =>
        fetch(`/api/series?genre=${encodeURIComponent(g.name)}&limit=4`)
          .then(r => r.json())
          .then(d => [g.name, d.items ?? []] as const)
          .catch(() => [g.name, []] as const)
      )
    ).then(pairs => setGenreCovers(Object.fromEntries(pairs)));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Series browser
  const [seriesFilter, setSeriesFilter] = useState<GenreFilter>(null);
  const [excludedGenres, setExcludedGenres] = useState<string[]>([]);
  const [genreFilterMode, setGenreFilterMode] = useState<GenreFilterMode>("include");
  const [libraryFilter, setLibraryFilter] = useState<string[]>([]);
  const [filteredLibrariesTotal, setFilteredLibrariesTotal] = useState(initialTotalSeries);
  const [browserGenres, setBrowserGenres] = useState<GenreDto[]>(initialGenres);
  const [seriesList, setSeriesList] = useState<SeriesDto[]>(initialUntagged);
  const [seriesTotal, setSeriesTotal] = useState(initialUntagged.length);
  const [untaggedCount, setUntaggedCount] = useState(initialUntagged.length);
  const [seriesLoading, setSeriesLoading] = useState(false);
  const [seriesSearch, setSeriesSearch] = useState("");
  const [seriesView, setSeriesView] = useState<SeriesView>("cards");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [assignInput, setAssignInput] = useState("");
  const filterRef = useRef<HTMLDivElement>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const refreshGenres = useCallback(async () => {
    const [res, ...libraryResponses] = await Promise.all([
      fetch("/api/genres"),
      ...(libraryFilter.length > 0
        ? libraryFilter.map(libraryId => fetch(`/api/genres?library_id=${libraryId}`))
        : [fetch("/api/genres")]),
    ]);
    if (res.ok) setGenres(await res.json());
    const genreGroups = await Promise.all(libraryResponses.map(async response => response.ok ? response.json() as Promise<GenreDto[]> : []));
    setBrowserGenres(mergeGenreCounts(genreGroups));
  }, [libraryFilter]);

  const fetchSeriesForFilter = useCallback(async (genre: GenreFilter, libraryIds: string[], excluded: string[] = []) => {
    setSeriesLoading(true);
    setSelected(new Set());
    setSeriesSearch("");
    try {
      const requestedLibraries = libraryIds.length > 0 ? libraryIds : [null];
      if (genre === null) {
        const results = await Promise.all(requestedLibraries.map(async libraryId => {
          const params = new URLSearchParams();
          if (libraryId) params.set("library_id", libraryId);
          const response = await fetch(`/api/genres/untagged-series?${params}`);
          return response.ok ? response.json() as Promise<SeriesDto[]> : [];
        }));
        const series = results.flat();
        setSeriesList(series);
        setSeriesTotal(series.length);
        setUntaggedCount(series.length);
      } else {
        const genreFilters = genre.length > 0 ? genre : [null];
        const results = await Promise.all(requestedLibraries.flatMap(libraryId => genreFilters.map(async genreName => {
          const params = new URLSearchParams({ limit: "500" });
          if (genreName) params.set("genre", genreName);
          if (libraryId) params.set("library_id", libraryId);
          const response = await fetch(`/api/series?${params}`);
          return response.ok ? response.json() as Promise<{ items: SeriesDto[] }> : { items: [] };
        })));
        const series = Array.from(new Map(results.flatMap(result => result.items).map(item => [item.series_id, item])).values())
          .filter(series => !excluded.some(excludedGenre => series.genres.includes(excludedGenre)));
        setSeriesList(series);
        setSeriesTotal(series.length);
      }
    } finally {
      setSeriesLoading(false);
    }
  }, []);

  const handleFilterChange = (genres: GenreFilter, excluded = excludedGenres) => {
    setSeriesFilter(genres);
    setExcludedGenres(excluded);
    fetchSeriesForFilter(genres, libraryFilter, excluded);
    filterRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  };

  const toggleGenreFilter = (genre: string) => {
    if (genreFilterMode === "exclude") {
      handleFilterChange(seriesFilter, excludedGenres.includes(genre)
        ? excludedGenres.filter(excludedGenre => excludedGenre !== genre)
        : [...excludedGenres, genre]);
      return;
    }
    const selectedGenres = Array.isArray(seriesFilter) ? seriesFilter : [];
    handleFilterChange(selectedGenres.includes(genre)
      ? selectedGenres.filter(selectedGenre => selectedGenre !== genre)
      : [...selectedGenres, genre]);
  };

  const fetchBrowserGenres = useCallback(async (libraryIds: string[]) => {
    const requestedLibraries = libraryIds.length > 0 ? libraryIds : [null];
    const results = await Promise.all(requestedLibraries.map(async libraryId => {
      const qs = libraryId ? `?library_id=${libraryId}` : "";
      const [genresRes, untaggedRes, seriesRes] = await Promise.all([
        fetch(`/api/genres${qs}`),
        fetch(`/api/genres/untagged-series${qs}`),
        fetch(`/api/series${qs}${qs ? "&" : "?"}limit=1`),
      ]);
      return {
        genres: genresRes.ok ? await genresRes.json() as GenreDto[] : [],
        untagged: untaggedRes.ok ? await untaggedRes.json() as SeriesDto[] : [],
        total: seriesRes.ok ? (await seriesRes.json() as { total: number }).total : 0,
      };
    }));
    setBrowserGenres(mergeGenreCounts(results.map(result => result.genres)));
    setUntaggedCount(results.reduce((count, result) => count + result.untagged.length, 0));
    setFilteredLibrariesTotal(results.reduce((count, result) => count + result.total, 0));
  }, []);

  const handleLibraryChange = (libraryIds: string[]) => {
    setLibraryFilter(libraryIds);
    fetchSeriesForFilter(seriesFilter, libraryIds, excludedGenres);
    fetchBrowserGenres(libraryIds);
  };

  const handleRename = async (oldName: string) => {
    const newName = renameValue.trim();
    if (!newName || newName === oldName) { setRenaming(null); return; }
    setBusy(true);
    try {
      await fetch(`/api/genres/${encodeURIComponent(oldName)}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ new_name: newName }),
      });
      if (Array.isArray(seriesFilter) && seriesFilter.includes(oldName)) {
        setSeriesFilter(seriesFilter.map(genre => genre === oldName ? newName : genre));
      }
      if (excludedGenres.includes(oldName)) {
        setExcludedGenres(excludedGenres.map(genre => genre === oldName ? newName : genre));
      }
      setGenreCovers(prev => {
        const next = { ...prev };
        next[newName] = next[oldName] ?? [];
        delete next[oldName];
        return next;
      });
      setRenaming(null);
      setRenameValue("");
      await refreshGenres();
      showToast(t("genres.renameSuccess"));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (name: string) => {
    if (!confirm(t("genres.deleteConfirm"))) return;
    setBusy(true);
    try {
      await fetch(`/api/genres/${encodeURIComponent(name)}`, { method: "DELETE" });
      setGenreCovers(prev => {
        const next = { ...prev };
        delete next[name];
        return next;
      });
      if (Array.isArray(seriesFilter) && seriesFilter.includes(name)) {
        const nextFilters = seriesFilter.filter(genre => genre !== name);
        setSeriesFilter(nextFilters);
        await Promise.all([refreshGenres(), fetchSeriesForFilter(nextFilters, libraryFilter, excludedGenres)]);
      } else {
        await refreshGenres();
      }
      showToast(t("genres.deleteSuccess"));
    } finally {
      setBusy(false);
    }
  };

  const handleAssign = async () => {
    const genre = assignInput.trim();
    if (!genre || selected.size === 0) return;
    setBusy(true);
    try {
      await fetch("/api/genres/assign", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ genre, series_ids: Array.from(selected) }),
      });
      const count = selected.size;
      await Promise.all([refreshGenres(), fetchSeriesForFilter(seriesFilter, libraryFilter, excludedGenres)]);
      setAssignInput("");
      showToast(t("genres.assignSuccess", { count: String(count), plural: count !== 1 ? "s" : "" }));
    } finally {
      setBusy(false);
    }
  };

  const toggleSelect = (id: string) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const displayedSeries = seriesSearch
    ? seriesList.filter(s => s.name.toLowerCase().includes(seriesSearch.toLowerCase()))
    : seriesList;

  const allSelected = displayedSeries.length > 0 && displayedSeries.every(s => selected.has(s.series_id));

  const toggleAll = () => {
    if (allSelected) {
      setSelected(prev => {
        const next = new Set(prev);
        displayedSeries.forEach(s => next.delete(s.series_id));
        return next;
      });
    } else {
      setSelected(prev => {
        const next = new Set(prev);
        displayedSeries.forEach(s => next.add(s.series_id));
        return next;
      });
    }
  };

  const filteredGenres = (genreFilter
    ? genres.filter(g => g.name.toLowerCase().includes(genreFilter.toLowerCase()))
    : genres
  ).sort((a, b) => b.series_count - a.series_count);
  const availableGenreNames = browserGenres.filter(g => g.series_count > 0).map(g => g.name);
  const activeGenreFilters = genreFilterMode === "include"
    ? (Array.isArray(seriesFilter) ? seriesFilter : [])
    : excludedGenres;

  // Stats
  const tagged = totalSeries > 0 ? totalSeries - untaggedCount : 0;
  const pct = totalSeries > 0 ? Math.round((tagged / totalSeries) * 100) : 0;
  const topGenre = [...genres].sort((a, b) => b.series_count - a.series_count)[0];

  return (
    <div className="space-y-10">
      {/* Toast */}
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 bg-success text-success-foreground px-4 py-3 rounded-xl shadow-lg text-sm font-medium">
          {toast}
        </div>
      )}

      {/* Genre detail modal */}
      {activeGenreModal && (
        <GenreSeriesModal genre={activeGenreModal} onClose={() => setActiveGenreModal(null)} />
      )}

      {/* ── Stats banner ─────────────────────────────────────────────── */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <StatCard emoji="🏷️" label={t("genres.allGenres")} value={genres.length} color="violet" />
        <StatCard
          emoji="✅"
          label={t("genres.taggedSeries")}
          value={tagged}
          sub={totalSeries > 0 ? `sur ${totalSeries} au total` : undefined}
          color="green"
        />
        <StatCard emoji="📈" label={t("genres.tagRate")} value={`${pct}%`} color="blue" />
        <StatCard
          emoji="⭐"
          label={t("genres.topGenre")}
          value={topGenre?.name ?? "—"}
          sub={topGenre ? `${topGenre.series_count} séries` : undefined}
          color="amber"
        />
      </div>

      {/* ── Genre cards ──────────────────────────────────────────────── */}
      <section>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 sm:gap-4 mb-5">
          <h2 className="text-lg font-semibold shrink-0">
            {t("genres.allGenres")} <span className="text-muted-foreground font-normal text-base">({genres.length})</span>
          </h2>
          {genres.length > 8 && (
            <input
              type="text"
              className="h-8 px-3 rounded-lg border border-border bg-background text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary w-full sm:w-56"
              placeholder={t("common.search") + "…"}
              value={genreFilter}
              onChange={e => setGenreFilter(e.target.value)}
            />
          )}
        </div>
        {genres.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("genres.noGenres")}</p>
        ) : (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
            {filteredGenres.map(g => {
              const covers = genreCovers[g.name] ?? [];
              return (
                <div key={g.name} className="group relative bg-card rounded-2xl border border-border overflow-hidden hover:border-primary/40 hover:shadow-lg transition-all duration-200">
                  {/* Cover hero */}
                  <button onClick={() => setActiveGenreModal(g.name)} className="w-full block focus:outline-none" tabIndex={-1}>
                    <div className="h-14 relative bg-muted overflow-hidden">
                      {covers.length > 0 ? (
                        <SeriesCoverImage series={covers[Math.floor(covers.length * (g.name.length % 4) / 4)]} />
                      ) : (
                        <div className="absolute inset-0 flex items-center justify-center text-muted-foreground/25">
                          <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                          </svg>
                        </div>
                      )}
                      <div className="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent" />
                    </div>
                  </button>

                  {/* Card body */}
                  <div className="px-2.5 py-2 min-w-0">
                    {renaming === g.name ? (
                      <div className="flex flex-wrap items-center gap-1.5 sm:flex-nowrap">
                        <input
                          className="h-6 min-w-0 w-full flex-1 bg-transparent text-sm focus:outline-none text-foreground border-b border-primary sm:w-auto"
                          value={renameValue}
                          autoFocus
                          onChange={e => setRenameValue(e.target.value)}
                          onKeyDown={e => {
                            if (e.key === "Enter") handleRename(g.name);
                            if (e.key === "Escape") { setRenaming(null); setRenameValue(""); }
                          }}
                        />
                        <button onClick={() => handleRename(g.name)} disabled={busy} className="text-primary hover:text-primary/70 disabled:opacity-50 shrink-0" title={t("common.save")}>
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" /></svg>
                        </button>
                        <button onClick={() => { setRenaming(null); setRenameValue(""); }} className="text-muted-foreground hover:text-foreground shrink-0" title={t("common.cancel")}>
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" /></svg>
                        </button>
                      </div>
                    ) : (
                      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-1.5">
                        <button
                          onClick={() => setActiveGenreModal(g.name)}
                          className="min-w-0 text-left font-semibold text-sm leading-tight text-foreground hover:text-primary transition-colors break-words sm:flex-1 sm:truncate sm:whitespace-nowrap"
                        >
                          {g.name}
                        </button>
                        <div className="flex items-center justify-between gap-2 sm:justify-end">
                          <span className="text-xs font-medium text-muted-foreground bg-muted px-1.5 py-0.5 rounded-full shrink-0">
                            {g.series_count}
                          </span>
                          <div className="flex items-center gap-1">
                            <button
                              onClick={() => { setRenaming(g.name); setRenameValue(g.name); }}
                              disabled={busy}
                              title={t("genres.rename")}
                              className="p-1 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors shrink-0"
                            >
                              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" /></svg>
                            </button>
                            <button
                              onClick={() => handleDelete(g.name)}
                              disabled={busy}
                              title={t("genres.delete")}
                              className="p-1 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive disabled:opacity-50 transition-colors shrink-0"
                            >
                              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                            </button>
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* ── Series browser ──────────────────────────────────────────── */}
      <section ref={filterRef}>
        <div className="flex items-center justify-between gap-4 mb-4">
          <h2 className="text-lg font-semibold">{t("nav.series")}</h2>
          {libraries.length > 1 && (
            <LibraryMultiBadgeSelector
              libraries={libraries}
              selectedIds={libraryFilter}
              onChange={handleLibraryChange}
            />
          )}
        </div>

        {/* Filter tabs */}
        <div className="flex flex-wrap items-center gap-2 mb-4">
          <div className="flex items-center rounded-lg border border-border p-0.5">
            <button
              type="button"
              onClick={() => setGenreFilterMode("include")}
              className={`rounded-md px-2 py-1 text-xs font-medium transition-colors ${genreFilterMode === "include" ? "bg-success/15 text-success" : "text-muted-foreground hover:text-foreground"}`}
            >
              {t("genres.includeGenres")}
            </button>
            <button
              type="button"
              onClick={() => setGenreFilterMode("exclude")}
              className={`rounded-md px-2 py-1 text-xs font-medium transition-colors ${genreFilterMode === "exclude" ? "bg-destructive/10 text-destructive" : "text-muted-foreground hover:text-foreground"}`}
            >
              {t("genres.excludeGenres")}
            </button>
          </div>
          <button
            onClick={() => {
              const allGenres = availableGenreNames;
              handleFilterChange(genreFilterMode === "include" ? allGenres : seriesFilter, genreFilterMode === "exclude" ? allGenres : excludedGenres);
            }}
            className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
              activeGenreFilters.length === availableGenreNames.length
                ? "bg-foreground text-background border-foreground"
                : "border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground"
            }`}
          >
            {t("common.all")}
            <span className="ml-1.5 opacity-70">
              ({filteredLibrariesTotal - untaggedCount})
            </span>
          </button>
          <button
            onClick={() => handleFilterChange(genreFilterMode === "include" ? [] : seriesFilter, genreFilterMode === "exclude" ? [] : excludedGenres)}
            className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
              activeGenreFilters.length === 0
                ? "bg-foreground text-background border-foreground"
                : "border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground"
            }`}
          >
            {t("genres.clearGenreFilters")}
            <span className="ml-1.5 opacity-70">
              ({Array.isArray(seriesFilter) && seriesFilter.length === 0 ? seriesTotal : filteredLibrariesTotal})
            </span>
          </button>
          <button
            onClick={() => handleFilterChange(null)}
            className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
              seriesFilter === null
                ? "bg-foreground text-background border-foreground"
                : "border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground"
            }`}
          >
            {t("genres.untaggedSeries")}
            <span className="ml-1.5 opacity-70">
              ({seriesFilter === null ? seriesTotal : untaggedCount})
            </span>
          </button>
          {[...browserGenres].sort((a, b) => b.series_count - a.series_count).filter(g => g.series_count > 0).map(g => (
            <button
              key={g.name}
              onClick={() => toggleGenreFilter(g.name)}
              className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                (genreFilterMode === "include" ? Array.isArray(seriesFilter) && seriesFilter.includes(g.name) : excludedGenres.includes(g.name))
                  ? genreFilterMode === "include" ? "bg-success/15 text-success border-success/40" : "bg-destructive/10 text-destructive border-destructive/40"
                  : "border-border text-muted-foreground hover:border-success/30 hover:text-foreground"
              }`}
            >
              {g.name}
              <span className="ml-1.5 opacity-70">
                ({g.series_count})
              </span>
            </button>
          ))}
        </div>

        {/* Toolbar */}
        <div className="flex flex-wrap items-center gap-3 mb-4">
          <input
            type="text"
            className="h-8 px-3 rounded-lg border border-border bg-background text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary flex-1 min-w-[160px] max-w-xs"
            placeholder={t("common.search") + "…"}
            value={seriesSearch}
            onChange={e => setSeriesSearch(e.target.value)}
          />
          <button onClick={toggleAll} className="text-xs font-medium text-primary hover:underline shrink-0">
            {allSelected ? t("genres.deselectAll") : t("genres.selectAll")}
          </button>
          <div className="flex items-center rounded-lg border border-border p-0.5">
            <button
              type="button"
              onClick={() => setSeriesView("cards")}
              title={t("genres.cardView")}
              aria-label={t("genres.cardView")}
              className={`rounded-md p-1.5 transition-colors ${seriesView === "cards" ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground"}`}
            >
              <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <rect x="3" y="3" width="7" height="7" rx="1" strokeWidth={2} />
                <rect x="14" y="3" width="7" height="7" rx="1" strokeWidth={2} />
                <rect x="3" y="14" width="7" height="7" rx="1" strokeWidth={2} />
                <rect x="14" y="14" width="7" height="7" rx="1" strokeWidth={2} />
              </svg>
            </button>
            <button
              type="button"
              onClick={() => setSeriesView("table")}
              title={t("genres.tableView")}
              aria-label={t("genres.tableView")}
              className={`rounded-md p-1.5 transition-colors ${seriesView === "table" ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground"}`}
            >
              <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
          </div>
          {selected.size > 0 && (
            <>
              <span className="text-xs text-muted-foreground shrink-0">{selected.size} sél.</span>
              <input
                type="text"
                className="h-8 px-3 rounded-lg border border-border bg-background text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary w-44"
                placeholder={t("genres.assignGenrePlaceholder")}
                value={assignInput}
                onChange={e => setAssignInput(e.target.value)}
                onKeyDown={e => { if (e.key === "Enter") handleAssign(); }}
                list="genre-suggestions"
              />
              <datalist id="genre-suggestions">
                {genres.map(g => <option key={g.name} value={g.name} />)}
              </datalist>
              <button
                onClick={handleAssign}
                disabled={busy || !assignInput.trim()}
                className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 disabled:opacity-50 transition-colors shrink-0"
              >
                {t("genres.assignButton", { count: String(selected.size), plural: selected.size !== 1 ? "s" : "" })}
              </button>
            </>
          )}
        </div>

        {/* Series cover grid */}
        {seriesLoading ? (
          <div className="py-12 text-center text-muted-foreground text-sm">{t("common.loading")}</div>
        ) : displayedSeries.length === 0 ? (
          <div className="py-12 text-center text-muted-foreground text-sm">
            {seriesFilter === null ? t("genres.noUntagged") : t("common.noData")}
          </div>
        ) : seriesView === "cards" ? (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8 gap-3">
            {displayedSeries.map(s => {
              const isSelected = selected.has(s.series_id);
              return (
                <div
                  key={s.series_id}
                  onClick={() => toggleSelect(s.series_id)}
                  className={`relative cursor-pointer rounded-xl overflow-hidden border-2 transition-all ${
                    isSelected
                      ? "border-primary ring-2 ring-primary/30"
                      : "border-border hover:border-primary/40"
                  }`}
                >
                  <div className="aspect-[2/3] relative bg-muted">
                    <SeriesCoverImage series={s} />
                    {!s.first_book_id && !s.cover_url && (
                      <div className="absolute inset-0 flex items-center justify-center text-muted-foreground text-xs p-2 text-center">{s.name}</div>
                    )}
                    {isSelected && (
                      <div className="absolute inset-0 bg-primary/20 flex items-center justify-center">
                        <div className="w-6 h-6 rounded-full bg-primary flex items-center justify-center">
                          <svg className="w-4 h-4 text-primary-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                          </svg>
                        </div>
                      </div>
                    )}
                  </div>
                  <div className="p-1.5">
                    <Link
                      href={`/series/${s.series_id}`}
                      onClick={e => e.stopPropagation()}
                      className="text-xs font-medium line-clamp-2 hover:text-primary transition-colors"
                    >
                      {s.name}
                    </Link>
                    {s.genres.length > 0 && (
                      <div className="mt-1 flex flex-wrap gap-1">
                        {s.genres.map(genre => (
                          <span
                            key={genre}
                            className="rounded-full bg-success/10 px-1.5 py-0.5 text-[10px] leading-tight text-success"
                          >
                            {genre}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="overflow-x-auto rounded-xl border border-border">
            <table className="w-full min-w-[620px] text-sm">
              <thead className="bg-muted/50 text-left text-xs uppercase tracking-wide text-muted-foreground">
                <tr>
                  <th className="w-10 px-3 py-3">
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={toggleAll}
                      aria-label={allSelected ? t("genres.deselectAll") : t("genres.selectAll")}
                      className="h-4 w-4 accent-primary"
                    />
                  </th>
                  <th className="w-14 px-2 py-3" aria-label="" />
                  <th className="px-3 py-3">{t("nav.series")}</th>
                  <th className="px-3 py-3">{t("nav.genres")}</th>
                  <th className="px-3 py-3 text-right">{t("dashboard.books")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {displayedSeries.map(s => {
                  const isSelected = selected.has(s.series_id);
                  return (
                    <tr
                      key={s.series_id}
                      onClick={() => toggleSelect(s.series_id)}
                      className={`cursor-pointer transition-colors ${isSelected ? "bg-primary/10" : "hover:bg-muted/60"}`}
                    >
                      <td className="px-3 py-2">
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onClick={e => e.stopPropagation()}
                          onChange={() => toggleSelect(s.series_id)}
                          aria-label={s.name}
                          className="h-4 w-4 accent-primary"
                        />
                      </td>
                      <td className="px-2 py-2">
                        <div className="relative h-12 w-9 overflow-hidden rounded-md bg-muted">
                          <SeriesCoverImage series={s} />
                        </div>
                      </td>
                      <td className="px-3 py-2 font-medium">
                        <Link href={`/series/${s.series_id}`} onClick={e => e.stopPropagation()} className="hover:text-primary transition-colors">
                          {s.name}
                        </Link>
                      </td>
                      <td className="px-3 py-2">
                        <div className="flex flex-wrap gap-1">
                          {s.genres.map(genre => (
                            <span key={genre} className="rounded-full bg-success/10 px-1.5 py-0.5 text-xs text-success">
                              {genre}
                            </span>
                          ))}
                        </div>
                      </td>
                      <td className="px-3 py-2 text-right text-muted-foreground">{s.book_count}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}

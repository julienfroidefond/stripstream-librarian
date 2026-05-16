"use client";

import { useState, useCallback, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import Image from "next/image";
import Link from "next/link";
import { getBookCoverUrl } from "@/lib/api";
import type { SeriesDto, LibraryDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

export type GenreDto = {
  name: string;
  series_count: number;
};

type Props = {
  initialGenres: GenreDto[];
  initialUntagged: SeriesDto[];
  libraries: LibraryDto[];
};

// null = "sans genre"
type GenreFilter = string | null;

function SeriesCoverImage({ series }: { series: SeriesDto }) {
  if (series.first_book_id) {
    return (
      <Image
        src={getBookCoverUrl(series.first_book_id, series.first_book_updated_at)}
        alt={series.name}
        fill
        className="object-cover"
        sizes="56px"
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

function GenreSeriesModal({
  genre,
  onClose,
}: {
  genre: string;
  onClose: () => void;
}) {
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

export function GenresClient({ initialGenres, initialUntagged, libraries }: Props) {
  const { t } = useTranslation();
  const [genres, setGenres] = useState<GenreDto[]>(initialGenres);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [activeGenreModal, setActiveGenreModal] = useState<string | null>(null);
  const [genreFilter, setGenreFilter] = useState("");

  // Series browser
  const [seriesFilter, setSeriesFilter] = useState<GenreFilter>(null); // null = sans genre
  const [libraryFilter, setLibraryFilter] = useState<string | null>(null);
  const [browserGenres, setBrowserGenres] = useState<GenreDto[]>(initialGenres);
  const [seriesList, setSeriesList] = useState<SeriesDto[]>(initialUntagged);
  const [seriesTotal, setSeriesTotal] = useState(initialUntagged.length);
  const [untaggedCount, setUntaggedCount] = useState(initialUntagged.length);
  const [seriesLoading, setSeriesLoading] = useState(false);
  const [seriesSearch, setSeriesSearch] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [assignInput, setAssignInput] = useState("");
  const filterRef = useRef<HTMLDivElement>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const refreshGenres = useCallback(async () => {
    const [res, res2] = await Promise.all([
      fetch("/api/genres"),
      fetch(libraryFilter ? `/api/genres?library_id=${libraryFilter}` : "/api/genres"),
    ]);
    if (res.ok) setGenres(await res.json());
    if (res2.ok) setBrowserGenres(await res2.json());
  }, [libraryFilter]);

  const fetchSeriesForFilter = useCallback(async (genre: GenreFilter, libId: string | null) => {
    setSeriesLoading(true);
    setSelected(new Set());
    setSeriesSearch("");
    try {
      if (genre === null) {
        const params = new URLSearchParams();
        if (libId) params.set("library_id", libId);
        const res = await fetch(`/api/genres/untagged-series?${params}`);
        if (res.ok) {
          const data: SeriesDto[] = await res.json();
          setSeriesList(data);
          setSeriesTotal(data.length);
          setUntaggedCount(data.length);
        }
      } else {
        const params = new URLSearchParams({ genre, limit: "500" });
        if (libId) params.set("library_id", libId);
        const res = await fetch(`/api/series?${params}`);
        if (res.ok) {
          const data = await res.json();
          setSeriesList(data.items ?? []);
          setSeriesTotal(data.total ?? 0);
        }
      }
    } finally {
      setSeriesLoading(false);
    }
  }, []);

  const handleFilterChange = (genre: GenreFilter) => {
    setSeriesFilter(genre);
    fetchSeriesForFilter(genre, libraryFilter);
    filterRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  };

  const fetchBrowserGenres = useCallback(async (libId: string | null) => {
    const qs = libId ? `?library_id=${libId}` : "";
    const [genresRes, untaggedRes] = await Promise.all([
      fetch(`/api/genres${qs}`),
      fetch(`/api/genres/untagged-series${qs}`),
    ]);
    if (genresRes.ok) setBrowserGenres(await genresRes.json());
    if (untaggedRes.ok) {
      const data: SeriesDto[] = await untaggedRes.json();
      setUntaggedCount(data.length);
    }
  }, []);

  const handleLibraryChange = (libId: string | null) => {
    setLibraryFilter(libId);
    fetchSeriesForFilter(seriesFilter, libId);
    fetchBrowserGenres(libId);
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
      if (seriesFilter === oldName) setSeriesFilter(newName);
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
      if (seriesFilter === name) {
        setSeriesFilter(null);
        await Promise.all([refreshGenres(), fetchSeriesForFilter(null, libraryFilter)]);
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
      await Promise.all([refreshGenres(), fetchSeriesForFilter(seriesFilter, libraryFilter)]);
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

  const filteredGenres = genreFilter
    ? genres.filter(g => g.name.toLowerCase().includes(genreFilter.toLowerCase()))
    : genres;

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

      {/* ── Genre pills ─────────────────────────────────────────────── */}
      <section>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 sm:gap-4 mb-4">
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
          <div className="grid grid-cols-2 sm:flex sm:flex-wrap gap-2">
            {filteredGenres.map((g) => (
              <div key={g.name} className="group relative">
                {renaming === g.name ? (
                  <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl border border-primary bg-background shadow-sm">
                    <input
                      className="h-6 min-w-0 flex-1 bg-transparent text-sm focus:outline-none text-foreground"
                      value={renameValue}
                      autoFocus
                      onChange={e => setRenameValue(e.target.value)}
                      onKeyDown={e => {
                        if (e.key === "Enter") handleRename(g.name);
                        if (e.key === "Escape") { setRenaming(null); setRenameValue(""); }
                      }}
                    />
                    <button onClick={() => handleRename(g.name)} disabled={busy} className="text-primary hover:text-primary/70 disabled:opacity-50" title={t("common.save")}>
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" /></svg>
                    </button>
                    <button onClick={() => { setRenaming(null); setRenameValue(""); }} className="text-muted-foreground hover:text-foreground" title={t("common.cancel")}>
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" /></svg>
                    </button>
                  </div>
                ) : (
                  <div className="flex items-center gap-1 pl-2.5 pr-1 py-1 rounded-xl border border-border bg-card hover:border-success/40 transition-colors w-full">
                    <button
                      onClick={() => setActiveGenreModal(g.name)}
                      className="flex items-center gap-1.5 text-sm font-medium text-foreground hover:text-success transition-colors min-w-0 flex-1"
                    >
                      <span className="w-2 h-2 rounded-full bg-success/60 shrink-0" />
                      <span className="truncate">{g.name}</span>
                      <span className="text-xs text-muted-foreground font-normal ml-0.5 shrink-0">{g.series_count}</span>
                    </button>
                    <div className="flex items-center gap-0.5 ml-1 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity">
                      <button onClick={() => { setRenaming(g.name); setRenameValue(g.name); }} disabled={busy} title={t("genres.rename")}
                        className="p-1 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors">
                        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" /></svg>
                      </button>
                      <button onClick={() => handleDelete(g.name)} disabled={busy} title={t("genres.delete")}
                        className="p-1 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive disabled:opacity-50 transition-colors">
                        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      {/* ── Series browser ──────────────────────────────────────────── */}
      <section ref={filterRef}>
        <div className="flex items-center justify-between gap-4 mb-4">
          <h2 className="text-lg font-semibold">{t("nav.series")}</h2>
          {libraries.length > 1 && (
            <select
              value={libraryFilter ?? ""}
              onChange={e => handleLibraryChange(e.target.value || null)}
              className="h-8 px-2 rounded-lg border border-border bg-background text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary"
            >
              <option value="">{t("common.all")}</option>
              {libraries.map(lib => (
                <option key={lib.id} value={lib.id}>{lib.name}</option>
              ))}
            </select>
          )}
        </div>

        {/* Filter tabs */}
        <div className="flex flex-wrap items-center gap-2 mb-4">
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
          {browserGenres.filter(g => g.series_count > 0).map(g => (
            <button
              key={g.name}
              onClick={() => handleFilterChange(g.name)}
              className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                seriesFilter === g.name
                  ? "bg-success/15 text-success border-success/40"
                  : "border-border text-muted-foreground hover:border-success/30 hover:text-foreground"
              }`}
            >
              {g.name}
              <span className="ml-1.5 opacity-70">
                ({seriesFilter === g.name ? seriesTotal : g.series_count})
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
        ) : (
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
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

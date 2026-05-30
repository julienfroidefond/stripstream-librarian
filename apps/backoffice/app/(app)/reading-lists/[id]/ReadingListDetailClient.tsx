"use client";

import { useState, useRef, useEffect, useCallback } from "react";
import Link from "next/link";
import { useTranslation } from "@/lib/i18n/context";
import type { ReadingListDetailDto, ReadingListSeriesDto, SeriesDto } from "@/lib/api";
import { Modal } from "@/app/components/ui/Modal";

type Props = {
  list: ReadingListDetailDto;
};

export function ReadingListDetailClient({ list: initialList }: Props) {
  const { t } = useTranslation();
  const [list, setList] = useState<ReadingListDetailDto>(initialList);
  const [items, setItems] = useState<ReadingListSeriesDto[]>(initialList.items);
  const [saving, setSaving] = useState(false);

  // Edit name/description
  const [editMode, setEditMode] = useState(false);
  const [editName, setEditName] = useState(list.name);
  const [editDescription, setEditDescription] = useState(list.description ?? "");

  // Add series modal
  const [showAdd, setShowAdd] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SeriesDto[]>([]);
  const [searching, setSearching] = useState(false);
  const searchTimeout = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const alreadyInList = new Set(items.map((i) => i.id));

  async function handleSaveEdit() {
    if (!editName.trim()) return;
    setSaving(true);
    try {
      const res = await fetch(`/api/reading-lists/${list.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: editName.trim(), description: editDescription.trim() || null }),
      });
      if (!res.ok) throw new Error();
      const updated = await res.json();
      setList((prev) => ({ ...prev, ...updated }));
      setEditMode(false);
    } finally {
      setSaving(false);
    }
  }

  const searchSeries = useCallback(
    (q: string) => {
      clearTimeout(searchTimeout.current);
      if (!q.trim()) { setSearchResults([]); return; }
      setSearching(true);
      searchTimeout.current = setTimeout(async () => {
        try {
          const res = await fetch(`/api/series?q=${encodeURIComponent(q)}&limit=20`);
          const data = await res.json();
          setSearchResults(data.items ?? []);
        } finally {
          setSearching(false);
        }
      }, 300);
    },
    []
  );

  useEffect(() => {
    searchSeries(searchQuery);
  }, [searchQuery, searchSeries]);

  async function handleAdd(series: SeriesDto) {
    try {
      await fetch(`/api/reading-lists/${list.id}/series`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ series_id: series.series_id }),
      });
      const newItem: ReadingListSeriesDto = {
        id: series.series_id,
        name: series.name,
        cover_url: series.cover_url,
        provider: series.metadata_provider,
        external_id: null,
        external_url: null,
        library_id: series.library_id,
        library_name: "",
        position: items.length,
      };
      setItems((prev) => [...prev, newItem]);
      alreadyInList.add(series.series_id);
    } catch {
      // silently fail
    }
  }

  async function handleRemove(seriesId: string) {
    try {
      await fetch(`/api/reading-lists/${list.id}/series/${seriesId}`, { method: "DELETE" });
      setItems((prev) => prev.filter((i) => i.id !== seriesId));
    } catch {
      // silently fail
    }
  }

  async function handleMove(index: number, direction: "up" | "down") {
    const newItems = [...items];
    const swapIndex = direction === "up" ? index - 1 : index + 1;
    if (swapIndex < 0 || swapIndex >= newItems.length) return;
    [newItems[index], newItems[swapIndex]] = [newItems[swapIndex], newItems[index]];
    setItems(newItems);

    try {
      await fetch(`/api/reading-lists/${list.id}/series/reorder`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ series_ids: newItems.map((i) => i.id) }),
      });
    } catch {
      // revert on error
      setItems(items);
    }
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Link
          href="/reading-lists"
          className="p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          title={t("readingLists.backToList")}
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 19l-7-7m0 0l7-7m-7 7h18" />
          </svg>
        </Link>
        <div className="flex-1 min-w-0">
          {editMode ? (
            <div className="space-y-2">
              <input
                autoFocus
                type="text"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                className="w-full text-2xl font-bold bg-transparent border-b border-primary focus:outline-none text-foreground"
              />
              <input
                type="text"
                value={editDescription}
                onChange={(e) => setEditDescription(e.target.value)}
                placeholder={t("readingLists.descriptionPlaceholder")}
                className="w-full text-sm bg-transparent border-b border-border focus:outline-none text-muted-foreground"
              />
              <div className="flex gap-2">
                <button
                  onClick={handleSaveEdit}
                  disabled={saving || !editName.trim()}
                  className="px-3 py-1 text-sm rounded-lg bg-primary text-primary-foreground disabled:opacity-50"
                >
                  {saving ? t("common.saving") : t("common.save")}
                </button>
                <button
                  onClick={() => { setEditMode(false); setEditName(list.name); setEditDescription(list.description ?? ""); }}
                  className="px-3 py-1 text-sm rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent"
                >
                  {t("common.cancel")}
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <div>
                <h1 className="text-2xl font-bold text-foreground">{list.name}</h1>
                {list.description && (
                  <p className="text-sm text-muted-foreground mt-0.5">{list.description}</p>
                )}
              </div>
              <button
                onClick={() => setEditMode(true)}
                className="p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                </svg>
              </button>
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={() => setShowAdd(true)}
          className="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors text-sm font-medium shrink-0"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          {t("readingLists.addSeries")}
        </button>
      </div>

      {/* Series list */}
      {items.length === 0 ? (
        <p className="text-muted-foreground text-center py-12">{t("readingLists.noSeries")}</p>
      ) : (
        <div className="space-y-2">
          {items.map((item, index) => (
            <div
              key={item.id}
              className="flex items-center gap-3 p-3 rounded-xl border border-border/50 bg-card"
            >
              <span className="text-xs text-muted-foreground w-6 text-center shrink-0">{index + 1}</span>

              {/* Cover */}
              <div className="w-10 h-14 rounded overflow-hidden shrink-0 bg-muted">
                {item.cover_url ? (
                  <img
                    src={item.cover_url}
                    alt={item.name}
                    className="w-full h-full object-cover"
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-muted-foreground/50">
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                    </svg>
                  </div>
                )}
              </div>

              {/* Info */}
              <div className="flex-1 min-w-0">
                <p className="font-medium text-foreground truncate">{item.name}</p>
                <p className="text-xs text-muted-foreground mt-0.5">{item.library_name}</p>
                {item.provider && (
                  <p className="text-xs text-muted-foreground/70">{item.provider}</p>
                )}
              </div>

              {/* Controls */}
              <div className="flex items-center gap-1 shrink-0">
                <button
                  onClick={() => handleMove(index, "up")}
                  disabled={index === 0}
                  className="p-1.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-30"
                  title={t("readingLists.moveUp")}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                  </svg>
                </button>
                <button
                  onClick={() => handleMove(index, "down")}
                  disabled={index === items.length - 1}
                  className="p-1.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-30"
                  title={t("readingLists.moveDown")}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </button>
                <button
                  onClick={() => handleRemove(item.id)}
                  className="p-1.5 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors ml-1"
                  title={t("readingLists.removeSeries")}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Add series modal */}
      <Modal
        isOpen={showAdd}
        onClose={() => { setShowAdd(false); setSearchQuery(""); setSearchResults([]); }}
        title={t("readingLists.addSeries")}
        maxWidth="md"
      >
        <div className="p-4 space-y-3">
          <input
            autoFocus
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("readingLists.searchSeries")}
            className="w-full px-3 py-2 rounded-lg border border-border bg-input text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          />

          {searching && (
            <p className="text-sm text-muted-foreground text-center py-2">{t("common.search")}...</p>
          )}

          {!searching && searchResults.length > 0 && (
            <div className="space-y-1 max-h-64 overflow-y-auto">
              {searchResults.map((series) => {
                const inList = alreadyInList.has(series.series_id);
                return (
                  <button
                    key={series.series_id}
                    type="button"
                    disabled={inList}
                    onClick={() => !inList && handleAdd(series)}
                    className="w-full flex items-center gap-3 p-2 rounded-lg hover:bg-accent transition-colors text-left disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <div className="w-8 h-11 rounded overflow-hidden shrink-0 bg-muted">
                      {series.cover_url ? (
                        <img src={series.cover_url} alt={series.name} className="w-full h-full object-cover" />
                      ) : (
                        <div className="w-full h-full bg-muted" />
                      )}
                    </div>
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-foreground truncate">{series.name}</p>
                      {inList && (
                        <p className="text-xs text-muted-foreground">✓ déjà dans la liste</p>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          )}

          {!searching && searchQuery.trim() && searchResults.length === 0 && (
            <p className="text-sm text-muted-foreground text-center py-4">Aucun résultat</p>
          )}
        </div>
      </Modal>
    </div>
  );
}

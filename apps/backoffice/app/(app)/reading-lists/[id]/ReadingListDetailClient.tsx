"use client";

import { useState, useRef, useCallback } from "react";
import Link from "next/link";
import { useTranslation } from "@/lib/i18n/context";
import type { ReadingListDetailDto, SeriesDto } from "@/lib/api";
import { getBookCoverUrl } from "@/lib/api";
import { browserApi } from "@/lib/api/browser";
import { Modal } from "@/app/components/ui/Modal";
import { ProviderIcon } from "@/app/components/ProviderIcon";
import { ReadingListsProvider, useReadingLists } from "@/app/components/ReadingListsProvider";

type Props = {
  list: ReadingListDetailDto;
};

/** The detail page: the list lives in the reading-lists store, not in local state. */
export function ReadingListDetailClient({ list }: Props) {
  return (
    <ReadingListsProvider detail={list}>
      <ReadingListDetailView initialList={list} />
    </ReadingListsProvider>
  );
}

function ReadingListDetailView({ initialList }: { initialList: ReadingListDetailDto }) {
  const { t } = useTranslation();
  const readingLists = useReadingLists();
  const list = readingLists.detail(initialList.id) ?? initialList;
  const items = list.items;
  const saving = readingLists.pending(`list:${list.id}`);

  const [editMode, setEditMode] = useState(false);
  const [editName, setEditName] = useState(list.name);
  const [editDescription, setEditDescription] = useState(list.description ?? "");

  const [showAdd, setShowAdd] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SeriesDto[]>([]);
  const [searching, setSearching] = useState(false);
  const searchTimeout = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const alreadyInList = new Set(items.map((i) => i.id));

  async function handleSaveEdit() {
    if (!editName.trim() || saving) return;
    const outcome = await readingLists.run({
      type: "rename",
      listId: list.id,
      name: editName.trim(),
      description: editDescription.trim() || null,
    });
    // On failure the form stays open so the reader can retry.
    if (outcome.ok) setEditMode(false);
  }

  const searchSeries = useCallback((q: string) => {
    clearTimeout(searchTimeout.current);
    if (!q.trim()) { setSearchResults([]); return; }
    setSearching(true);
    searchTimeout.current = setTimeout(async () => {
      try {
        const data = await browserApi.series.list({ q, limit: 20 });
        setSearchResults(data.items ?? []);
      } catch {
        setSearchResults([]);
      } finally {
        setSearching(false);
      }
    }, 300);
  }, []);

  function handleAdd(series: SeriesDto) {
    // The API answers 204: the new row appears once the store re-reads the list.
    void readingLists.run({ type: "add", listId: list.id, seriesId: series.series_id });
  }

  function handleRemove(seriesId: string) {
    void readingLists.run({ type: "remove", listId: list.id, seriesId });
  }

  function handleMove(seriesId: string, direction: "up" | "down") {
    void readingLists.run({ type: "move", listId: list.id, seriesId, direction });
  }

  return (
    <div className="space-y-6 max-w-3xl mx-auto">
      {/* Header */}
      <div className="flex items-start gap-3">
        <Link
          href="/reading-lists"
          className="mt-1 p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors shrink-0"
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
                onKeyDown={(e) => { if (e.key === "Enter") handleSaveEdit(); if (e.key === "Escape") setEditMode(false); }}
                className="w-full text-2xl font-bold bg-transparent border-b-2 border-primary focus:outline-none text-foreground"
              />
              <input
                type="text"
                value={editDescription}
                onChange={(e) => setEditDescription(e.target.value)}
                placeholder={t("readingLists.descriptionPlaceholder")}
                className="w-full text-sm bg-transparent border-b border-border focus:outline-none text-muted-foreground"
              />
              <div className="flex gap-2 pt-1">
                <button
                  onClick={handleSaveEdit}
                  disabled={saving || !editName.trim()}
                  className="px-3 py-1.5 text-sm rounded-lg bg-primary text-primary-foreground disabled:opacity-50 font-medium"
                >
                  {saving ? t("common.saving") : t("common.save")}
                </button>
                <button
                  onClick={() => { setEditMode(false); setEditName(list.name); setEditDescription(list.description ?? ""); }}
                  className="px-3 py-1.5 text-sm rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent"
                >
                  {t("common.cancel")}
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-start gap-2">
              <div className="min-w-0">
                <h1 className="text-2xl font-bold text-foreground leading-tight">{list.name}</h1>
                {list.description && (
                  <p className="text-sm text-muted-foreground mt-1">{list.description}</p>
                )}
                <p className="text-xs text-muted-foreground/60 mt-1.5">
                  {t("readingLists.seriesCount", {
                    count: items.length,
                    plural: items.length !== 1 ? "s" : "",
                  })}
                </p>
              </div>
              <button
                onClick={() => setEditMode(true)}
                title={t("readingLists.editTitle")}
                aria-label={t("readingLists.editTitle")}
                className="mt-0.5 p-1.5 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors shrink-0"
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
          className="mt-0.5 flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors text-sm font-medium shrink-0"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          <span className="hidden sm:inline">{t("readingLists.addSeries")}</span>
          <span className="sm:hidden">{t("common.add")}</span>
        </button>
      </div>

      {/* Series list */}
      {items.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 gap-4 text-muted-foreground border border-dashed border-border/60 rounded-xl">
          <svg className="w-12 h-12 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
          <p className="text-sm">{t("readingLists.noSeries")}</p>
        </div>
      ) : (
        <div className="space-y-1.5">
          {items.map((item, index) => (
            <div
              key={item.id}
              className="group flex items-center gap-3 p-2.5 rounded-xl border border-border/40 bg-card hover:border-border/80 hover:bg-accent/20 transition-all duration-150"
            >
              {/* Position */}
              <span className="text-xs font-mono text-muted-foreground/50 w-5 text-right shrink-0 select-none">
                {index + 1}
              </span>

              {/* Cover */}
              <Link href={`/series/${item.id}`} className="shrink-0">
                <div className="w-16 h-24 rounded-lg overflow-hidden bg-muted shadow-sm">
                  {(item.first_book_id || item.cover_url) ? (
                    <img
                      src={item.first_book_id ? getBookCoverUrl(item.first_book_id, item.first_book_updated_at) : (item.cover_url ?? "")}
                      alt={item.name}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center">
                      <svg className="w-5 h-5 text-muted-foreground/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                      </svg>
                    </div>
                  )}
                </div>
              </Link>

              {/* Info */}
              <Link href={`/series/${item.id}`} className="flex-1 min-w-0">
                <p className="font-semibold text-foreground text-sm leading-snug truncate">{item.name}</p>
                <p className="text-xs text-muted-foreground/70 mt-0.5 truncate">{item.library_name}</p>
                {item.provider && (
                  <div className="flex items-center gap-1.5 mt-1">
                    <ProviderIcon provider={item.provider} size={12} />
                    <span className="text-xs text-muted-foreground/50">{item.provider}</span>
                  </div>
                )}
                {item.is_empty && (
                  <span className="inline-block mt-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-500/10 text-amber-600 dark:text-amber-400">
                    {t("readingLists.emptySeries")}
                  </span>
                )}
              </Link>

              {/* Controls */}
              <div className="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={() => handleMove(item.id, "up")}
                  disabled={index === 0}
                  className="p-1.5 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-20 disabled:cursor-not-allowed"
                  title={t("readingLists.moveUp")}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                  </svg>
                </button>
                <button
                  onClick={() => handleMove(item.id, "down")}
                  disabled={index === items.length - 1}
                  className="p-1.5 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-20 disabled:cursor-not-allowed"
                  title={t("readingLists.moveDown")}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </button>
                <button
                  onClick={() => handleRemove(item.id)}
                  disabled={readingLists.pending(`item:${list.id}:${item.id}`)}
                  className="p-1.5 rounded-lg text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors ml-0.5"
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
            onChange={(e) => { setSearchQuery(e.target.value); searchSeries(e.target.value); }}
            placeholder={t("readingLists.searchSeries")}
            className="w-full px-3 py-2 rounded-lg border border-border bg-input text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          />

          {searching && (
            <p className="text-sm text-muted-foreground text-center py-4">{t("common.search")}...</p>
          )}

          {!searching && searchResults.length > 0 && (
            <div className="space-y-1 max-h-72 overflow-y-auto -mx-1 px-1">
              {searchResults.map((series) => {
                const inList = alreadyInList.has(series.series_id);
                const adding = readingLists.pending(`item:${list.id}:${series.series_id}`);
                return (
                  <button
                    key={series.series_id}
                    type="button"
                    disabled={inList || adding}
                    onClick={() => !inList && !adding && handleAdd(series)}
                    className="w-full flex items-center gap-3 p-2.5 rounded-lg hover:bg-accent transition-colors text-left disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    <div className="w-9 h-12 rounded-md overflow-hidden shrink-0 bg-muted shadow-sm">
                      {(series.first_book_id || series.cover_url) ? (
                        <img
                          src={series.first_book_id ? getBookCoverUrl(series.first_book_id, series.first_book_updated_at) : (series.cover_url ?? "")}
                          alt={series.name}
                          className="w-full h-full object-cover"
                        />
                      ) : (
                        <div className="w-full h-full bg-muted" />
                      )}
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium text-foreground truncate">{series.name}</p>
                      {inList ? (
                        <p className="text-xs text-muted-foreground">{t("readingLists.alreadyInList")}</p>
                      ) : series.metadata_provider ? (
                        <div className="flex items-center gap-1 mt-0.5">
                          <ProviderIcon provider={series.metadata_provider} size={11} />
                          <span className="text-xs text-muted-foreground/60">{series.metadata_provider}</span>
                        </div>
                      ) : null}
                    </div>
                    {!inList && (
                      <svg className="w-4 h-4 text-muted-foreground/50 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                      </svg>
                    )}
                  </button>
                );
              })}
            </div>
          )}

          {!searching && searchQuery.trim() && searchResults.length === 0 && (
            <p className="text-sm text-muted-foreground text-center py-6">{t("readingLists.noResults")}</p>
          )}
        </div>
      </Modal>
    </div>
  );
}

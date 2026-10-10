"use client";

import { useState, useCallback, useRef } from "react";
import { useRouter } from "next/navigation";
import { Button, Icon, Modal } from "./ui";
import { useTranslation } from "@/lib/i18n/context";

type SeriesResult = {
  series_id: string;
  name: string;
  book_count: number;
  metadata_provider: string | null;
};

interface MergeSeriesButtonProps {
  seriesId: string;
  seriesName: string;
  children?: (open: () => void) => React.ReactNode;
}

export function MergeSeriesButton({
  seriesId,
  seriesName,
  children,
}: MergeSeriesButtonProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState(seriesName);
  const [results, setResults] = useState<SeriesResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [selected, setSelected] = useState<SeriesResult | null>(null);
  const [merging, setMerging] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const search = useCallback(
    async (q: string) => {
      if (q.length < 2) {
        setResults([]);
        return;
      }
      setSearching(true);
      try {
        const params = new URLSearchParams({
          q,
          page: "1",
          limit: "20",
        });
        const resp = await fetch(`/api/series/search?${params}`);
        const data = await resp.json();
        if (resp.ok) {
          setResults(
            (data.items ?? []).filter(
              (s: SeriesResult) => s.series_id !== seriesId,
            ),
          );
        } else {
          console.error("[merge search]", data);
        }
      } catch {
        setResults([]);
      } finally {
        setSearching(false);
      }
    },
    [seriesId],
  );

  function handleQueryChange(value: string) {
    setQuery(value);
    setSelected(null);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => search(value), 300);
  }

  async function handleMerge() {
    if (!selected) return;
    setMerging(true);
    try {
      const resp = await fetch(`/api/series/${seriesId}/merge`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ source_id: selected.series_id }),
      });
      if (resp.ok) {
        setIsOpen(false);
        router.refresh();
      }
    } catch {
      // error
    } finally {
      setMerging(false);
    }
  }

  function handleClose() {
    setIsOpen(false);
    setQuery(seriesName);
    setResults([]);
    setSelected(null);
  }

  const open = () => { setIsOpen(true); setQuery(seriesName); search(seriesName); };

  return (
    <>
      {children ? (
        children(open)
      ) : (
        <button
          type="button"
          onClick={open}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-foreground text-sm font-medium hover:bg-muted transition-colors"
        >
          <Icon name="merge" size="sm" />
          {t("seriesDetail.merge")}
        </button>
      )}

      <Modal
        isOpen={isOpen}
        onClose={handleClose}
        maxWidth="md"
        title={t("seriesDetail.mergeTitle")}
      >
        <div className="p-6 space-y-4">
          <p className="text-sm text-muted-foreground">
            {t("seriesDetail.mergeDescription")}
          </p>

          <input
            type="text"
            value={query}
            onChange={(e) => handleQueryChange(e.target.value)}
            placeholder={t("seriesDetail.mergeSearch")}
            className="w-full px-3 py-2 rounded-lg border border-border bg-background text-foreground text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary"
            autoFocus
          />

          <div className="max-h-64 overflow-y-auto space-y-1">
            {searching && (
              <div className="flex items-center justify-center py-4">
                <Icon
                  name="spinner"
                  size="sm"
                  className="animate-spin text-muted-foreground"
                />
              </div>
            )}
            {!searching && query.length >= 2 && results.length === 0 && (
              <p className="text-sm text-muted-foreground text-center py-4">
                {t("seriesDetail.mergeNoResults")}
              </p>
            )}
            {results.map((s) => (
              <button
                key={s.series_id}
                type="button"
                onClick={() => setSelected(s)}
                className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
                  selected?.series_id === s.series_id
                    ? "bg-primary/10 border border-primary/30 text-foreground"
                    : "hover:bg-muted text-foreground"
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="font-medium">{s.name}</span>
                  <span className="text-xs text-muted-foreground">
                    {s.book_count}{" "}
                    {s.book_count === 1 ? "livre" : "livres"}
                    {s.metadata_provider && (
                      <span className="ml-1.5 text-primary">
                        {s.metadata_provider}
                      </span>
                    )}
                  </span>
                </div>
              </button>
            ))}
          </div>
        </div>

        <div className="flex justify-end gap-2 px-6 pb-6">
          <Button variant="outline" size="sm" onClick={handleClose}>
            {t("common.cancel")}
          </Button>
          <Button
            size="sm"
            onClick={handleMerge}
            disabled={!selected || merging}
          >
            {merging ? (
              <Icon name="spinner" size="sm" className="animate-spin" />
            ) : (
              <Icon name="merge" size="sm" />
            )}
            {t("seriesDetail.mergeConfirm")}
          </Button>
        </div>
      </Modal>
    </>
  );
}

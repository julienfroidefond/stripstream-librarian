"use client";

import { useState, useEffect } from "react";
import { Modal } from "./ui/Modal";
import { useTranslation } from "@/lib/i18n/context";
import type { ReadingListDto } from "@/lib/api";
import { getBookCoverUrl } from "@/lib/api";

interface Props {
  seriesId: string;
  seriesName: string;
  children?: (open: () => void) => React.ReactNode;
}

export function AddToReadingListModal({ seriesId, children }: Props) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [lists, setLists] = useState<ReadingListDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [adding, setAdding] = useState<string | null>(null);
  const [added, setAdded] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!isOpen) return;
    fetch("/api/reading-lists")
      .then((r) => r.json())
      .then((data: ReadingListDto[]) => setLists(data))
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [isOpen]);

  async function handleAdd(listId: string) {
    setAdding(listId);
    try {
      const res = await fetch(`/api/reading-lists/${listId}/series`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ series_id: seriesId }),
      });
      if (res.ok || res.status === 400) {
        setAdded((prev) => new Set([...prev, listId]));
      }
    } finally {
      setAdding(null);
    }
  }

  const open = () => { setIsOpen(true); setAdded(new Set()); setLoading(true); };

  return (
    <>
      {children ? children(open) : (
        <button type="button" onClick={open} className="text-sm text-muted-foreground hover:text-foreground">
          {t("readingLists.addToList")}
        </button>
      )}
      <Modal
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        title={t("readingLists.addToList")}
        maxWidth="sm"
      >
        <div className="p-4">
          {loading ? (
            <p className="text-sm text-muted-foreground text-center py-6">{t("common.loading")}</p>
          ) : lists.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-6">{t("readingLists.empty")}</p>
          ) : (
            <div className="space-y-1 max-h-72 overflow-y-auto">
              {lists.map((list) => {
                const isAdded = added.has(list.id);
                const isAdding = adding === list.id;
                return (
                  <button
                    key={list.id}
                    type="button"
                    onClick={() => !isAdded && !isAdding && handleAdd(list.id)}
                    disabled={isAdding}
                    className="w-full flex items-center gap-3 p-2.5 rounded-lg hover:bg-accent transition-colors text-left disabled:opacity-60"
                  >
                    {/* Mini cover mosaic */}
                    <div className="w-10 h-10 rounded-lg overflow-hidden shrink-0 bg-muted">
                      {list.preview_covers[0] ? (
                        <img src={getBookCoverUrl(list.preview_covers[0])} alt="" className="w-full h-full object-cover" />
                      ) : (
                        <div className="w-full h-full flex items-center justify-center">
                          <svg className="w-4 h-4 text-muted-foreground/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                          </svg>
                        </div>
                      )}
                    </div>

                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-foreground truncate">{list.name}</p>
                      <p className="text-xs text-muted-foreground/60">
                        {t("readingLists.seriesCount", { count: list.series_count, plural: list.series_count !== 1 ? "s" : "" })}
                      </p>
                    </div>

                    <div className="shrink-0">
                      {isAdded ? (
                        <svg className="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                        </svg>
                      ) : isAdding ? (
                        <svg className="w-4 h-4 text-muted-foreground animate-spin" fill="none" viewBox="0 0 24 24">
                          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                        </svg>
                      ) : (
                        <svg className="w-4 h-4 text-muted-foreground/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                        </svg>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </Modal>
    </>
  );
}

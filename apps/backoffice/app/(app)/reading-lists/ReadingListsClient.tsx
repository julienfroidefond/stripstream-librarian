"use client";

import { useState } from "react";
import Link from "next/link";
import { useTranslation } from "@/lib/i18n/context";
import type { ReadingListDto } from "@/lib/api";
import { Modal } from "@/app/components/ui/Modal";
import { ReadingListCover } from "@/app/components/ReadingListCover";

type Props = {
  initialLists: ReadingListDto[];
};


export function ReadingListsClient({ initialLists }: Props) {
  const { t } = useTranslation();
  const [lists, setLists] = useState<ReadingListDto[]>(initialLists);
  const [showCreate, setShowCreate] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createDescription, setCreateDescription] = useState("");
  const [saving, setSaving] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  async function handleCreate() {
    if (!createName.trim()) return;
    setSaving(true);
    try {
      const res = await fetch("/api/reading-lists", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: createName.trim(), description: createDescription.trim() || undefined }),
      });
      if (!res.ok) throw new Error();
      const created: ReadingListDto = await res.json();
      setLists((prev) => [...prev, created].sort((a, b) => a.name.localeCompare(b.name)));
      setShowCreate(false);
      setCreateName("");
      setCreateDescription("");
    } catch {
      // silently fail
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(e: React.MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    if (!confirm(t("readingLists.deleteConfirm"))) return;
    setDeletingId(id);
    try {
      await fetch(`/api/reading-lists/${id}`, { method: "DELETE" });
      setLists((prev) => prev.filter((l) => l.id !== id));
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-cyan-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
          {t("readingLists.title")}
        </h1>
        <button
          type="button"
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors text-sm font-medium"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          {t("readingLists.create")}
        </button>
      </div>

      {lists.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-24 gap-4 text-muted-foreground">
          <svg className="w-16 h-16 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
          <p className="text-sm">{t("readingLists.empty")}</p>
        </div>
      ) : (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
          {lists.map((list) => (
            <Link
              key={list.id}
              href={`/reading-lists/${list.id}`}
              className="group relative flex flex-col rounded-xl overflow-hidden border border-border/50 bg-card hover:border-border hover:shadow-lg transition-all duration-200"
            >
              {/* Cover mosaic */}
              <div className="relative">
                <ReadingListCover covers={list.preview_covers} name={list.name} />
                {/* Hover overlay with delete button */}
                <div className="absolute inset-0 bg-black/0 group-hover:bg-black/20 transition-colors duration-200" />
                <button
                  type="button"
                  onClick={(e) => handleDelete(e, list.id)}
                  disabled={deletingId === list.id}
                  className="absolute top-2 right-2 p-1.5 rounded-lg bg-background/80 text-muted-foreground hover:text-destructive hover:bg-destructive/10 opacity-0 group-hover:opacity-100 transition-all duration-200 backdrop-blur-sm"
                  title={t("common.delete")}
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>

              {/* Info */}
              <div className="p-3 flex flex-col gap-1">
                <p className="font-semibold text-foreground text-sm leading-tight line-clamp-2">{list.name}</p>
                {list.description && (
                  <p className="text-xs text-muted-foreground line-clamp-1">{list.description}</p>
                )}
                <p className="text-xs text-muted-foreground/70 mt-0.5">
                  {t("readingLists.seriesCount", {
                    count: list.series_count,
                    plural: list.series_count !== 1 ? "s" : "",
                  })}
                </p>
              </div>
            </Link>
          ))}
        </div>
      )}

      <Modal
        isOpen={showCreate}
        onClose={() => { setShowCreate(false); setCreateName(""); setCreateDescription(""); }}
        title={t("readingLists.createTitle")}
        maxWidth="sm"
        disableClose={saving}
      >
        <div className="p-5 space-y-4">
          <div>
            <label className="text-sm font-medium text-foreground mb-1.5 block">
              {t("readingLists.name")}
            </label>
            <input
              autoFocus
              type="text"
              value={createName}
              onChange={(e) => setCreateName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder={t("readingLists.namePlaceholder")}
              className="w-full px-3 py-2 rounded-lg border border-border bg-input text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </div>
          <div>
            <label className="text-sm font-medium text-foreground mb-1.5 block">
              {t("readingLists.description")}
            </label>
            <textarea
              value={createDescription}
              onChange={(e) => setCreateDescription(e.target.value)}
              placeholder={t("readingLists.descriptionPlaceholder")}
              rows={2}
              className="w-full px-3 py-2 rounded-lg border border-border bg-input text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-primary resize-none"
            />
          </div>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={() => { setShowCreate(false); setCreateName(""); setCreateDescription(""); }}
              className="px-4 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              onClick={handleCreate}
              disabled={saving || !createName.trim()}
              className="px-4 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              {saving ? t("common.saving") : t("readingLists.create")}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

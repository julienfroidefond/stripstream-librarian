"use client";

import { useState } from "react";
import { useTranslation } from "@/lib/i18n/context";
import type { ReadingListDto } from "@/lib/api";
import { Modal } from "@/app/components/ui/Modal";
import { ReadingListCard } from "@/app/components/ReadingListCard";
import { Button, Card } from "@/app/components/ui";

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
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-cyan-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
          {t("readingLists.title")}
        </h1>
        <Button
          type="button"
          onClick={() => setShowCreate(true)}
          className="shrink-0 gap-2"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          {t("readingLists.create")}
        </Button>
      </div>

      {lists.length === 0 ? (
        <Card hover={false} className="border-dashed">
          <div className="flex flex-col items-center justify-center py-20 gap-4 text-muted-foreground">
            <svg className="w-14 h-14 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
            </svg>
            <p className="text-sm">{t("readingLists.empty")}</p>
            <Button type="button" variant="outline" size="sm" onClick={() => setShowCreate(true)}>{t("readingLists.create")}</Button>
          </div>
        </Card>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {lists.map((list) => (
            <ReadingListCard
              key={list.id}
              list={list}
              seriesCountLabel={t("readingLists.seriesCount", { count: list.series_count, plural: list.series_count !== 1 ? "s" : "" })}
              booksLabel={t("dashboard.books")}
              readLabel={t("status.read")}
              action={(
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={(e) => handleDelete(e, list.id)}
                  disabled={deletingId === list.id}
                  className="shrink-0 px-2 text-muted-foreground hover:text-destructive"
                  title={t("common.delete")}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </Button>
              )}
            />
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

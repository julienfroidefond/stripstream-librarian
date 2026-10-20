"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "@/lib/i18n/context";
import type {
  AiListAdditionSuggestion,
  AiListCreationSuggestion,
  AiReadingListSuggestions,
  AiSuggestedSeries,
} from "@/lib/api";
import { getBookCoverUrl } from "@/lib/api";
import { Button, Icon } from "@/app/components/ui";
import { Modal } from "@/app/components/ui/Modal";
import { toast } from "@/app/components/ui/Toast";
import { coverSrc } from "@/app/components/ReadingListCover";

type Props = {
  isOpen: boolean;
  onClose: () => void;
  /** Called after at least one proposal was applied, so the page can refresh. */
  onApplied: () => Promise<void> | void;
};

type Status = "loading" | "ready" | "error";
type ListDraft = { name: string; description: string };

const addKey = (listId: string, seriesId: string) => `add:${listId}:${seriesId}`;
const createKey = (index: number, seriesId: string) => `new:${index}:${seriesId}`;

function seriesCover(series: AiSuggestedSeries): string | null {
  if (series.first_book_id) {
    return getBookCoverUrl(series.first_book_id, series.first_book_updated_at);
  }
  return series.cover_url ? coverSrc(series.cover_url) : null;
}

export function AiReadingListsModal({ isOpen, onClose, onApplied }: Props) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<Status>("loading");
  const [suggestions, setSuggestions] = useState<AiReadingListSuggestions | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  const [excluded, setExcluded] = useState<Set<string>>(new Set());
  const [drafts, setDrafts] = useState<Record<number, ListDraft>>({});
  const [applying, setApplying] = useState(false);
  const requestedRef = useRef(false);
  const requestIdRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setStatus("loading");
    setSuggestions(null);
    setExcluded(new Set());
    setDrafts({});
    setErrorMessage("");
    try {
      const response = await fetch("/api/reading-lists/ai-suggest", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      const payload = await response.json().catch(() => null);
      if (requestId !== requestIdRef.current) return;
      if (!response.ok) {
        throw new Error(
          response.status === 422
            ? t("readingLists.aiError")
            : payload?.error ?? t("readingLists.aiError")
        );
      }
      const data = payload as AiReadingListSuggestions;
      setSuggestions(data);
      setDrafts(
        Object.fromEntries(
          data.new_lists.map((list, index) => [
            index,
            { name: list.name, description: list.description },
          ])
        )
      );
      setStatus("ready");
    } catch (error) {
      if (requestId !== requestIdRef.current) return;
      setErrorMessage(error instanceof Error ? error.message : t("readingLists.aiError"));
      setStatus("error");
    }
  }, [t]);

  useEffect(() => {
    if (!isOpen) {
      requestedRef.current = false;
      // Invalidate an in-flight request so a late response cannot repopulate
      // the modal after it was closed.
      requestIdRef.current += 1;
      return;
    }
    if (requestedRef.current) return;
    requestedRef.current = true;
    void load();
  }, [isOpen, load]);

  const additions = useMemo(() => suggestions?.additions ?? [], [suggestions]);
  const newLists = useMemo(() => suggestions?.new_lists ?? [], [suggestions]);

  const toggleKey = (key: string) => {
    setExcluded((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const toggleGroup = (keys: string[]) => {
    setExcluded((previous) => {
      const next = new Set(previous);
      const allSelected = keys.every((key) => !next.has(key));
      for (const key of keys) {
        if (allSelected) next.add(key);
        else next.delete(key);
      }
      return next;
    });
  };

  const selection = useMemo(() => {
    const additionSeries = additions.flatMap((addition) =>
      addition.series
        .filter((series) => !excluded.has(addKey(addition.list_id, series.id)))
        .map((series) => ({ addition, series }))
    );
    const selectedAdditionGroups = additions.filter((addition) =>
      addition.series.some((series) => !excluded.has(addKey(addition.list_id, series.id)))
    );
    const creations = newLists
      .map((list, index) => ({
        list,
        index,
        series: list.series.filter((series) => !excluded.has(createKey(index, series.id))),
        draft: drafts[index] ?? { name: list.name, description: list.description },
      }))
      // A new list is only valid with at least two series, a name and a
      // description: keep the UI consistent with what the API accepts.
      .filter(
        ({ series, draft }) =>
          series.length >= 2 &&
          draft.name.trim().length > 0 &&
          draft.description.trim().length > 0
      );

    return {
      additionSeries,
      creations,
      seriesCount:
        additionSeries.length + creations.reduce((total, creation) => total + creation.series.length, 0),
      actionCount: selectedAdditionGroups.length + creations.length,
    };
  }, [additions, newLists, excluded, drafts]);

  const updateDraft = (index: number, patch: Partial<ListDraft>) => {
    setDrafts((previous) => ({
      ...previous,
      [index]: { ...previous[index], ...patch },
    }));
  };

  const handleApply = async () => {
    if (!suggestions || selection.actionCount === 0) return;
    setApplying(true);
    let failures = 0;
    let applied = 0;

    for (const { addition, series } of selection.additionSeries) {
      try {
        const response = await fetch(`/api/reading-lists/${addition.list_id}/series`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ series_id: series.id }),
        });
        if (response.ok) applied += 1;
        else failures += 1;
      } catch {
        failures += 1;
      }
    }

    for (const creation of selection.creations) {
      const name = creation.draft.name.trim();
      const description = creation.draft.description.trim();
      try {
        const response = await fetch("/api/reading-lists", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ name, description }),
        });
        if (!response.ok) {
          failures += 1;
          continue;
        }
        const created = (await response.json()) as { id: string };
        let addedSeries = 0;
        for (const series of creation.series) {
          const addResponse = await fetch(`/api/reading-lists/${created.id}/series`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ series_id: series.id }),
          });
          if (addResponse.ok) {
            applied += 1;
            addedSeries += 1;
          } else failures += 1;
        }
        if (addedSeries > 0) {
          applied += 1;
        } else {
          // Do not leave an empty list behind when every series failed.
          failures += 1;
          await fetch(`/api/reading-lists/${created.id}`, { method: "DELETE" }).catch(() => {});
        }
      } catch {
        failures += 1;
      }
    }

    setApplying(false);
    if (applied > 0) {
      toast(failures > 0 ? t("readingLists.aiApplyError") : t("readingLists.aiApplied"), failures > 0 ? "error" : "success");
      await onApplied();
      onClose();
    } else {
      toast(t("readingLists.aiApplyFailed"), "error");
    }
  };

  const empty = status === "ready" && additions.length === 0 && newLists.length === 0;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t("readingLists.aiTitle")}
      maxWidth="5xl"
      disableClose={applying}
      footer={
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-sm text-muted-foreground">
            {t("readingLists.aiSeriesCount", {
              count: selection.seriesCount,
              plural: selection.seriesCount !== 1 ? "s" : "",
            })}
          </p>
          <div className="flex items-center gap-2">
            <Button type="button" variant="ghost" onClick={onClose} disabled={applying}>
              {t("common.cancel")}
            </Button>
            <Button
              type="button"
              onClick={handleApply}
              disabled={applying || status !== "ready" || selection.actionCount === 0}
            >
              {applying
                ? t("readingLists.aiApplying")
                : t("readingLists.aiApply", { count: selection.actionCount })}
            </Button>
          </div>
        </div>
      }
    >
      <div className="space-y-6 p-5">
        <p className="text-sm leading-relaxed text-muted-foreground">{t("readingLists.aiDescription")}</p>

        {status === "loading" && (
          <div className="flex flex-col items-center justify-center gap-3 py-16 text-muted-foreground">
            <Icon name="spinner" size="lg" className="animate-spin" />
            <p className="text-sm">{t("readingLists.aiLoading")}</p>
          </div>
        )}

        {status === "error" && (
          <div className="flex flex-col items-center justify-center gap-4 rounded-2xl border border-destructive/30 bg-destructive/5 px-6 py-12 text-center">
            <Icon name="warning" size="lg" className="text-destructive" />
            <p className="max-w-lg text-sm text-destructive">{errorMessage || t("readingLists.aiError")}</p>
            <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
              {t("readingLists.aiRetry")}
            </Button>
          </div>
        )}

        {empty && (
          <div className="flex flex-col items-center justify-center gap-4 rounded-2xl border border-dashed border-border/60 px-6 py-12 text-center text-muted-foreground">
            <Icon name="bot" size="lg" className="opacity-40" />
            <p className="text-sm">{t("readingLists.aiEmpty")}</p>
            <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
              {t("readingLists.aiRetry")}
            </Button>
          </div>
        )}

        {status === "ready" && additions.length > 0 && (
          <section className="space-y-3">
            <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t("readingLists.aiAdditions")}
            </h4>
            {additions.map((addition, index) => (
              <AdditionCard
                key={`${addition.list_id}-${index}`}
                addition={addition}
                excluded={excluded}
                onToggle={toggleKey}
                onToggleGroup={toggleGroup}
              />
            ))}
          </section>
        )}

        {status === "ready" && newLists.length > 0 && (
          <section className="space-y-3">
            <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t("readingLists.aiNewLists")}
            </h4>
            {newLists.map((list, index) => (
              <CreationCard
                key={`${list.name}-${index}`}
                list={list}
                index={index}
                draft={drafts[index] ?? { name: list.name, description: list.description }}
                excluded={excluded}
                onToggle={toggleKey}
                onToggleGroup={toggleGroup}
                onDraftChange={updateDraft}
              />
            ))}
          </section>
        )}
      </div>
    </Modal>
  );
}

function AdditionCard({
  addition,
  excluded,
  onToggle,
  onToggleGroup,
}: {
  addition: AiListAdditionSuggestion;
  excluded: Set<string>;
  onToggle: (key: string) => void;
  onToggleGroup: (keys: string[]) => void;
}) {
  const keys = addition.series.map((series) => addKey(addition.list_id, series.id));
  const allSelected = keys.length > 0 && keys.every((key) => !excluded.has(key));
  const selectedCount = keys.filter((key) => !excluded.has(key)).length;

  return (
    <div className="rounded-2xl border border-border/60 bg-muted/10 p-4">
      <div className="flex items-start gap-3">
        <input
          type="checkbox"
          className="mt-1 h-4 w-4 shrink-0 accent-primary"
          checked={allSelected}
          onChange={() => onToggleGroup(keys)}
          aria-label={addition.list_name}
        />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
            <p className="font-semibold text-foreground">{addition.list_name}</p>
            <p className="text-xs text-muted-foreground">
              {selectedCount}/{addition.series.length}
            </p>
          </div>
          {addition.reason && (
            <p className="mt-0.5 text-sm leading-relaxed text-muted-foreground">{addition.reason}</p>
          )}
        </div>
      </div>
      <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {addition.series.map((series) => (
          <SeriesOption
            key={series.id}
            series={series}
            selected={!excluded.has(addKey(addition.list_id, series.id))}
            onToggle={() => onToggle(addKey(addition.list_id, series.id))}
          />
        ))}
      </div>
    </div>
  );
}

function CreationCard({
  list,
  index,
  draft,
  excluded,
  onToggle,
  onToggleGroup,
  onDraftChange,
}: {
  list: AiListCreationSuggestion;
  index: number;
  draft: ListDraft;
  excluded: Set<string>;
  onToggle: (key: string) => void;
  onToggleGroup: (keys: string[]) => void;
  onDraftChange: (index: number, patch: Partial<ListDraft>) => void;
}) {
  const { t } = useTranslation();
  const keys = list.series.map((series) => createKey(index, series.id));
  const allSelected = keys.length > 0 && keys.every((key) => !excluded.has(key));
  const selectedCount = keys.filter((key) => !excluded.has(key)).length;
  const missingName = draft.name.trim().length === 0;
  const missingDescription = draft.description.trim().length === 0;
  const invalid = selectedCount > 0 && (selectedCount < 2 || missingName || missingDescription);

  return (
    <div
      className={`rounded-2xl border bg-primary/5 p-4 ${
        invalid ? "border-destructive/40" : "border-primary/25"
      }`}
    >
      <div className="flex items-start gap-3">
        <input
          type="checkbox"
          className="mt-2 h-4 w-4 shrink-0 accent-primary"
          checked={allSelected}
          onChange={() => onToggleGroup(keys)}
          aria-label={draft.name || list.name}
        />
        <div className="min-w-0 flex-1 space-y-2">
          <input
            type="text"
            value={draft.name}
            onChange={(event) => onDraftChange(index, { name: event.target.value })}
            aria-label={t("readingLists.aiListName")}
            aria-invalid={missingName}
            placeholder={t("readingLists.aiListName")}
            className="w-full rounded-lg border border-border bg-input px-3 py-2 text-sm font-semibold text-foreground focus:outline-none focus:ring-2 focus:ring-primary"
          />
          <textarea
            value={draft.description}
            onChange={(event) => onDraftChange(index, { description: event.target.value })}
            aria-label={t("readingLists.aiListDescription")}
            aria-invalid={missingDescription}
            placeholder={t("readingLists.aiListDescription")}
            rows={2}
            className="w-full resize-none rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary"
          />
          {list.reason && (
            <p className="text-xs leading-relaxed text-muted-foreground">{list.reason}</p>
          )}
          <p className="text-xs text-muted-foreground">{selectedCount}/{list.series.length}</p>
          {invalid && (
            <p className="text-xs text-destructive" role="status">
              {t("readingLists.aiNewListInvalid")}
            </p>
          )}
        </div>
      </div>
      <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {list.series.map((series) => (
          <SeriesOption
            key={series.id}
            series={series}
            selected={!excluded.has(createKey(index, series.id))}
            onToggle={() => onToggle(createKey(index, series.id))}
          />
        ))}
      </div>
    </div>
  );
}

function SeriesOption({
  series,
  selected,
  onToggle,
}: {
  series: AiSuggestedSeries;
  selected: boolean;
  onToggle: () => void;
}) {
  const src = seriesCover(series);

  return (
    <label
      className={`flex cursor-pointer items-center gap-3 rounded-xl border p-2 transition-colors ${
        selected
          ? "border-primary/40 bg-background/80"
          : "border-border/40 bg-muted/20 opacity-55"
      }`}
    >
      <input
        type="checkbox"
        checked={selected}
        onChange={onToggle}
        className="h-4 w-4 shrink-0 accent-primary"
      />
      <div className="h-14 w-10 shrink-0 overflow-hidden rounded-md bg-muted">
        {src ? (
          <img src={src} alt="" className="h-full w-full object-cover" />
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            <Icon name="books" size="sm" className="text-muted-foreground/40" />
          </div>
        )}
      </div>
      <div className="min-w-0">
        <p className="truncate text-sm font-medium text-foreground">{series.name}</p>
        <p className="truncate text-xs text-muted-foreground">{series.library_name}</p>
      </div>
    </label>
  );
}

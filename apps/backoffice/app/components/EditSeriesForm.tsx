"use client";

import { useState, useTransition, useEffect, useLayoutEffect, useCallback, useMemo, useRef } from "react";
import { createPortal } from "react-dom";
import { Modal } from "./ui/Modal";
import { useRouter } from "next/navigation";
import { FormField, FormLabel, FormInput } from "./ui/Form";
import { Icon, TagInput } from "./ui";
import { LockButton } from "./LockButton";
import { pickLockableFields } from "@/lib/metadata/locks";
import { browserApi, browserErrorMessage, type SeriesUpdateRequest } from "@/lib/api/browser";
import { ApiError } from "@/lib/api/errors";
import { useLockableFields } from "./MetadataFieldsProvider";
import { useTranslation } from "@/lib/i18n/context";
import { seriesEditDefaults, type EditSeriesModel } from "@/lib/series/editForm";
import { translateSeriesStatus } from "@/lib/i18n/optional";

const SERIES_STATUS_VALUES = ["", "ongoing", "ended", "hiatus", "cancelled", "upcoming"] as const;

/** Autocomplete dropdown for the genre input. Rendered in a portal so it is not
 * clipped by the modal's scroll container. The anchor rect is measured in an
 * effect (never during render) and kept in sync on scroll/resize. */
function GenreSuggestions({
  anchorRef,
  items,
  onSelect,
}: {
  anchorRef: React.RefObject<HTMLInputElement | null>;
  items: string[];
  onSelect: (genre: string) => void;
}) {
  const [rect, setRect] = useState<DOMRect | null>(null);

  useLayoutEffect(() => {
    const update = () => setRect(anchorRef.current?.getBoundingClientRect() ?? null);
    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [anchorRef]);

  if (!rect) return null;

  return createPortal(
    <div
      style={{ position: "fixed", top: rect.bottom + 6, left: rect.left, width: rect.width, zIndex: 9999 }}
      className="overflow-hidden rounded-xl border border-border bg-card shadow-2xl"
    >
      <ul className="max-h-52 overflow-y-auto">
        {items.map((g) => (
          <li key={g}>
            <button
              type="button"
              onMouseDown={(e) => { e.preventDefault(); onSelect(g); }}
              className="w-full px-3 py-2.5 text-left text-sm text-foreground hover:bg-success/10 hover:text-success transition-colors flex items-center gap-2 group"
            >
              <span className="w-1.5 h-1.5 rounded-full bg-success/40 group-hover:bg-success transition-colors flex-shrink-0" />
              {g}
            </button>
          </li>
        ))}
      </ul>
    </div>,
    document.body,
  );
}

export interface EditSeriesFormProps {
  children?: (open: () => void) => React.ReactNode;
  /** The series detail view model (`SeriesActionsModel`, or the slice the form reads). */
  model: EditSeriesModel;
}

export function EditSeriesForm({ model, children }: EditSeriesFormProps) {
  const { seriesId, seriesName } = model;
  const defaults = useMemo(() => seriesEditDefaults(model), [model]);
  const { t } = useTranslation();
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Champs propres à la série
  const [newName, setNewName] = useState(defaults.name);
  const [authors, setAuthors] = useState<string[]>(defaults.authors);
  const [authorInput, setAuthorInput] = useState("");
  const [authorInputEl, setAuthorInputEl] = useState<HTMLInputElement | null>(null);
  const [genres, setGenres] = useState<string[]>(defaults.genres);
  const [genreInput, setGenreInput] = useState("");
  const genreInputRef = useRef<HTMLInputElement>(null);
  const [showGenreSuggestions, setShowGenreSuggestions] = useState(false);
  const [allGenres, setAllGenres] = useState<string[]>([]);
  const [publishers, setPublishers] = useState<string[]>(defaults.publishers);
  const [publisherInput, setPublisherInput] = useState("");
  const [publisherInputEl, setPublisherInputEl] = useState<HTMLInputElement | null>(null);
  const [description, setDescription] = useState(defaults.description);
  const [startYear, setStartYear] = useState(defaults.startYear);
  const [totalVolumes, setTotalVolumes] = useState(defaults.totalVolumes);
  const [status, setStatus] = useState(defaults.status);

  // Lock states. The raw map is kept as-is; the catalog decides which keys are
  // still meaningful for the lock UI and the PATCH payload.
  const lockableFields = useLockableFields("series");
  const [lockedFields, setLockedFields] = useState<Record<string, boolean>>(
    () => defaults.lockedFields,
  );
  const visibleLockedFields = pickLockableFields(lockedFields, lockableFields);

  // Propagation aux livres — opt-in via bouton
  const [bookAuthor, setBookAuthor] = useState(defaults.bookAuthor);
  const [bookLanguage, setBookLanguage] = useState(defaults.bookLanguage);
  const [showApplyToBooks, setShowApplyToBooks] = useState(false);

  const toggleLock = (field: string) => {
    setLockedFields((prev) => ({ ...prev, [field]: !prev[field] }));
  };

  const addAuthor = () => {
    const v = authorInput.trim();
    if (v && !authors.includes(v)) {
      setAuthors([...authors, v]);
    }
    setAuthorInput("");
    authorInputEl?.focus();
  };

  const removeAuthor = (idx: number) => {
    setAuthors(authors.filter((_, i) => i !== idx));
  };

  const handleAuthorKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addAuthor();
    }
  };

  const addGenre = (value?: string) => {
    const v = (value ?? genreInput).trim();
    if (v && !genres.includes(v)) {
      setGenres((prev) => [...prev, v]);
    }
    setGenreInput("");
    setShowGenreSuggestions(false);
    genreInputRef.current?.focus();
  };

  const removeGenre = (idx: number) => {
    setGenres(genres.filter((_, i) => i !== idx));
  };

  const handleGenreKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addGenre();
    }
  };

  const addPublisher = () => {
    const v = publisherInput.trim();
    if (v && !publishers.includes(v)) {
      setPublishers([...publishers, v]);
    }
    setPublisherInput("");
    publisherInputEl?.focus();
  };

  const removePublisher = (idx: number) => {
    setPublishers(publishers.filter((_, i) => i !== idx));
  };

  const handlePublisherKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addPublisher();
    }
  };

  const handleClose = useCallback(() => {
    setNewName(defaults.name);
    setAuthors(defaults.authors);
    setAuthorInput("");
    setGenres(defaults.genres);
    setGenreInput("");
    setShowGenreSuggestions(false);
    setPublishers(defaults.publishers);
    setPublisherInput("");
    setDescription(defaults.description);
    setStartYear(defaults.startYear);
    setTotalVolumes(defaults.totalVolumes);
    setStatus(defaults.status);
    setLockedFields(defaults.lockedFields);
    setShowApplyToBooks(false);
    setBookAuthor(defaults.bookAuthor);
    setBookLanguage(defaults.bookLanguage);
    setError(null);
    setIsOpen(false);
  }, [defaults]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isPending) handleClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, isPending, handleClose]);

  useEffect(() => {
    if (isOpen && allGenres.length === 0) {
      browserApi.series.genres().then(setAllGenres).catch(() => {});
    }
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!showGenreSuggestions) return;
    const onMouseDown = (e: MouseEvent) => {
      if (!genreInputRef.current?.contains(e.target as Node)) {
        setShowGenreSuggestions(false);
      }
    };
    document.addEventListener("mousedown", onMouseDown);
    return () => document.removeEventListener("mousedown", onMouseDown);
  }, [showGenreSuggestions]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim() && seriesName !== "unclassified") return;
    setError(null);

    const finalAuthors = authorInput.trim()
      ? [...new Set([...authors, authorInput.trim()])]
      : authors;

    const finalGenres = genreInput.trim()
      ? [...new Set([...genres, genreInput.trim()])]
      : genres;

    const finalPublishers = publisherInput.trim()
      ? [...new Set([...publishers, publisherInput.trim()])]
      : publishers;

    startTransition(async () => {
      try {
        const effectiveName = newName.trim() || "unclassified";
        const body: SeriesUpdateRequest = {
          new_name: effectiveName,
          authors: finalAuthors,
          genres: finalGenres,
          publishers: finalPublishers,
          description: description.trim() || null,
          start_year: startYear.trim() ? parseInt(startYear.trim(), 10) : null,
          total_volumes: totalVolumes.trim() ? parseInt(totalVolumes.trim(), 10) : null,
          status: status || null,
          locked_fields: visibleLockedFields,
        };
        if (showApplyToBooks) {
          body.author = bookAuthor.trim() || null;
          body.language = bookLanguage.trim() || null;
        }

        await browserApi.series.update(seriesId, body);
        setIsOpen(false);
        router.refresh();
      } catch (error) {
        if (error instanceof ApiError) {
          setError(browserErrorMessage(error, t("editBook.saveError")));
        } else {
          setError(t("common.networkError"));
        }
      }
    });
  };

  const modal = (
    <Modal isOpen={isOpen} onClose={handleClose} title={t("editSeries.title")} disableClose={isPending}>
          <form onSubmit={handleSubmit} className="p-5 space-y-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <FormField>
                <FormLabel required>{t("editSeries.name")}</FormLabel>
                <FormInput
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editSeries.namePlaceholder")}
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.startYear")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.start_year} onToggle={() => toggleLock("start_year")} disabled={isPending} />
                </div>
                <FormInput
                  type="number"
                  min="1900"
                  max="2100"
                  value={startYear}
                  onChange={(e) => setStartYear(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editSeries.startYearPlaceholder")}
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.totalVolumes")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.total_volumes} onToggle={() => toggleLock("total_volumes")} disabled={isPending} />
                </div>
                <FormInput
                  type="number"
                  min="1"
                  value={totalVolumes}
                  onChange={(e) => setTotalVolumes(e.target.value)}
                  disabled={isPending}
                  placeholder="12"
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.status")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.status} onToggle={() => toggleLock("status")} disabled={isPending} />
                </div>
                <select
                  value={status}
                  onChange={(e) => setStatus(e.target.value)}
                  disabled={isPending}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {SERIES_STATUS_VALUES.map((v) => (
                    <option key={v} value={v}>
                      {v === "" ? t("seriesStatus.notDefined") : translateSeriesStatus(t, v)}
                    </option>
                  ))}
                </select>
              </FormField>

              {/* Auteurs — multi-valeur */}
              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.authors")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.authors} onToggle={() => toggleLock("authors")} disabled={isPending} />
                </div>
                <TagInput
                  values={authors}
                  input={authorInput}
                  onInputChange={setAuthorInput}
                  onAdd={addAuthor}
                  onRemove={removeAuthor}
                  removeLabel={(name) => t("editBook.removeAuthor", { name })}
                  onKeyDown={handleAuthorKeyDown}
                  placeholder={t("editBook.addAuthor")}
                  disabled={isPending}
                  inputRef={setAuthorInputEl}
                  extra={
                    <button
                      type="button"
                      onClick={() => setShowApplyToBooks(!showApplyToBooks)}
                      disabled={isPending}
                      className={`px-3 py-1.5 rounded-lg border text-sm font-medium transition-colors ${
                        showApplyToBooks
                          ? "border-primary bg-primary/10 text-primary"
                          : "border-border bg-card text-muted-foreground hover:text-foreground"
                      }`}
                      title={t("editSeries.applyToBooksTitle")}
                    >
                      {t("editSeries.applyToBooks")}
                    </button>
                  }
                />
              </FormField>

              {showApplyToBooks && (
                <div className="sm:col-span-2 grid grid-cols-1 sm:grid-cols-2 gap-3 pl-4 border-l-2 border-primary/30">
                  <FormField>
                    <FormLabel>{t("editSeries.bookAuthor")}</FormLabel>
                    <FormInput
                      value={bookAuthor}
                      onChange={(e) => setBookAuthor(e.target.value)}
                      disabled={isPending}
                      placeholder={t("editSeries.bookAuthorPlaceholder")}
                    />
                  </FormField>
                  <FormField>
                    <FormLabel>{t("editSeries.bookLanguage")}</FormLabel>
                    <FormInput
                      value={bookLanguage}
                      onChange={(e) => setBookLanguage(e.target.value)}
                      disabled={isPending}
                      placeholder={t("editBook.languagePlaceholder")}
                    />
                  </FormField>
                </div>
              )}

              {/* Éditeurs — multi-valeur */}
              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.publishers")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.publishers} onToggle={() => toggleLock("publishers")} disabled={isPending} />
                </div>
                <TagInput
                  values={publishers}
                  input={publisherInput}
                  onInputChange={setPublisherInput}
                  onAdd={addPublisher}
                  onRemove={removePublisher}
                  removeLabel={(name) => t("editBook.removeAuthor", { name })}
                  onKeyDown={handlePublisherKeyDown}
                  placeholder={t("editSeries.addPublisher")}
                  disabled={isPending}
                  variant="secondary"
                  inputRef={setPublisherInputEl}
                />
              </FormField>

              {/* Genres — multi-valeur avec autocomplete */}
              <FormField className="sm:col-span-2">
                <FormLabel>{t("editSeries.genres")}</FormLabel>
                <TagInput
                  values={genres}
                  input={genreInput}
                  onInputChange={(v) => { setGenreInput(v); setShowGenreSuggestions(true); }}
                  onAdd={() => addGenre()}
                  onRemove={removeGenre}
                  removeLabel={(name) => t("editBook.removeAuthor", { name })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") { e.preventDefault(); addGenre(); }
                    if (e.key === "Escape") setShowGenreSuggestions(false);
                  }}
                  onFocus={() => setShowGenreSuggestions(true)}
                  placeholder={t("editSeries.addGenre")}
                  disabled={isPending}
                  variant="success"
                  autoComplete="off"
                  inputRef={(el) => { genreInputRef.current = el; }}
                  suggestions={showGenreSuggestions && (() => {
                    const q = genreInput.toLowerCase();
                    const items = allGenres.filter(
                      (g) => !genres.includes(g) && (q === "" || g.toLowerCase().includes(q))
                    );
                    if (!items.length) return null;
                    return <GenreSuggestions anchorRef={genreInputRef} items={items} onSelect={addGenre} />;
                  })()}
                />
              </FormField>

              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.description")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.description} onToggle={() => toggleLock("description")} disabled={isPending} />
                </div>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  disabled={isPending}
                  rows={3}
                  placeholder={t("editSeries.descriptionPlaceholder")}
                  className="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-none"
                />
              </FormField>
            </div>

            {/* Lock legend */}
            {Object.values(visibleLockedFields).some(Boolean) && (
              <p className="text-xs text-warning flex items-center gap-1.5">
                <Icon name="lock" size="sm" className="!w-3.5 !h-3.5 shrink-0" />
                {t("editBook.lockedFieldsNote")}
              </p>
            )}

            {error && <p className="text-xs text-destructive">{error}</p>}

            {/* Footer */}
            <div className="flex items-center justify-end gap-2 pt-2 border-t border-border/50">
              <button
                type="button"
                onClick={handleClose}
                disabled={isPending}
                className="px-4 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                {t("common.cancel")}
              </button>
              <button
                type="submit"
                disabled={isPending || (!newName.trim() && seriesName !== "unclassified")}
                className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isPending ? t("common.saving") : t("common.save")}
              </button>
            </div>
          </form>
    </Modal>
  );

  const open = () => setIsOpen(true);

  return (
    <>
      {children ? (
        children(open)
      ) : (
        <button
          onClick={open}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
        >
          <span>✏️</span> {t("editSeries.title")}
        </button>
      )}
      {modal}
    </>
  );
}

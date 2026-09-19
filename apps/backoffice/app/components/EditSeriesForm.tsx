"use client";

import { useState, useTransition, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { Modal } from "./ui/Modal";
import { useRouter } from "next/navigation";
import { FormField, FormLabel, FormInput } from "./ui/Form";
import { Icon, TagInput } from "./ui";
import { LockButton } from "./LockButton";
import { useTranslation } from "../../lib/i18n/context";

const SERIES_STATUS_VALUES = ["", "ongoing", "ended", "hiatus", "cancelled", "upcoming"] as const;

export interface EditSeriesFormProps {
  children?: (open: () => void) => React.ReactNode;
  libraryId: string;
  seriesId: string;
  seriesName: string;
  currentAuthors: string[];
  currentGenres: string[];
  currentPublishers: string[];
  currentBookAuthor: string | null;
  currentBookLanguage: string | null;
  currentDescription: string | null;
  currentStartYear: number | null;
  currentTotalVolumes: number | null;
  currentStatus: string | null;
  currentLockedFields: Record<string, boolean>;
}

export function EditSeriesForm({
  libraryId,
  seriesId,
  seriesName,
  currentAuthors,
  currentGenres,
  currentPublishers,
  currentBookAuthor,
  currentBookLanguage,
  currentDescription,
  currentStartYear,
  currentTotalVolumes,
  currentStatus,
  currentLockedFields,
  children,
}: EditSeriesFormProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Champs propres à la série
  const [newName, setNewName] = useState(seriesName === "unclassified" ? "" : seriesName);
  const [authors, setAuthors] = useState<string[]>(currentAuthors);
  const [authorInput, setAuthorInput] = useState("");
  const [authorInputEl, setAuthorInputEl] = useState<HTMLInputElement | null>(null);
  const [genres, setGenres] = useState<string[]>(currentGenres);
  const [genreInput, setGenreInput] = useState("");
  const genreInputRef = useRef<HTMLInputElement>(null);
  const [showGenreSuggestions, setShowGenreSuggestions] = useState(false);
  const [allGenres, setAllGenres] = useState<string[]>([]);
  const [publishers, setPublishers] = useState<string[]>(currentPublishers);
  const [publisherInput, setPublisherInput] = useState("");
  const [publisherInputEl, setPublisherInputEl] = useState<HTMLInputElement | null>(null);
  const [description, setDescription] = useState(currentDescription ?? "");
  const [startYear, setStartYear] = useState(currentStartYear?.toString() ?? "");
  const [totalVolumes, setTotalVolumes] = useState(currentTotalVolumes?.toString() ?? "");
  const [status, setStatus] = useState(currentStatus ?? "");

  // Lock states
  const [lockedFields, setLockedFields] = useState<Record<string, boolean>>(currentLockedFields);

  // Propagation aux livres — opt-in via bouton
  const [bookAuthor, setBookAuthor] = useState(currentBookAuthor ?? "");
  const [bookLanguage, setBookLanguage] = useState(currentBookLanguage ?? "");
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
    setNewName(seriesName === "unclassified" ? "" : seriesName);
    setAuthors(currentAuthors);
    setAuthorInput("");
    setGenres(currentGenres);
    setGenreInput("");
    setShowGenreSuggestions(false);
    setPublishers(currentPublishers);
    setPublisherInput("");
    setDescription(currentDescription ?? "");
    setStartYear(currentStartYear?.toString() ?? "");
    setTotalVolumes(currentTotalVolumes?.toString() ?? "");
    setStatus(currentStatus ?? "");
    setLockedFields(currentLockedFields);
    setShowApplyToBooks(false);
    setBookAuthor(currentBookAuthor ?? "");
    setBookLanguage(currentBookLanguage ?? "");
    setError(null);
    setIsOpen(false);
  }, [seriesName, currentAuthors, currentGenres, currentPublishers, currentDescription, currentStartYear, currentTotalVolumes, currentBookAuthor, currentBookLanguage, currentLockedFields]);

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
      fetch("/api/series/genres").then(r => r.json()).then(setAllGenres).catch(() => {});
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
        const body: Record<string, unknown> = {
          new_name: effectiveName,
          authors: finalAuthors,
          genres: finalGenres,
          publishers: finalPublishers,
          description: description.trim() || null,
          start_year: startYear.trim() ? parseInt(startYear.trim(), 10) : null,
          total_volumes: totalVolumes.trim() ? parseInt(totalVolumes.trim(), 10) : null,
          status: status || null,
          locked_fields: lockedFields,
        };
        if (showApplyToBooks) {
          body.author = bookAuthor.trim() || null;
          body.language = bookLanguage.trim() || null;
        }

        const res = await fetch(
          `/api/series/${seriesId}`,
          {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
          }
        );
        if (!res.ok) {
          const data = await res.json();
          setError(data.error ?? t("editBook.saveError"));
          return;
        }
        setIsOpen(false);
        router.refresh();
      } catch {
        setError(t("common.networkError"));
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
                  <LockButton locked={!!lockedFields.start_year} onToggle={() => toggleLock("start_year")} disabled={isPending} />
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
                  <LockButton locked={!!lockedFields.total_volumes} onToggle={() => toggleLock("total_volumes")} disabled={isPending} />
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
                  <LockButton locked={!!lockedFields.status} onToggle={() => toggleLock("status")} disabled={isPending} />
                </div>
                <select
                  value={status}
                  onChange={(e) => setStatus(e.target.value)}
                  disabled={isPending}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {SERIES_STATUS_VALUES.map((v) => (
                    <option key={v} value={v}>
                      {v === "" ? t("seriesStatus.notDefined") : t(`seriesStatus.${v}` as any)}
                    </option>
                  ))}
                </select>
              </FormField>

              {/* Auteurs — multi-valeur */}
              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.authors")}</FormLabel>
                  <LockButton locked={!!lockedFields.authors} onToggle={() => toggleLock("authors")} disabled={isPending} />
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
                  <LockButton locked={!!lockedFields.publishers} onToggle={() => toggleLock("publishers")} disabled={isPending} />
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
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editSeries.genres")}</FormLabel>
                  <LockButton locked={!!lockedFields.genres} onToggle={() => toggleLock("genres")} disabled={isPending} />
                </div>
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
                    const rect = genreInputRef.current?.getBoundingClientRect();
                    if (!rect) return null;
                    const q = genreInput.toLowerCase();
                    const suggestions = allGenres.filter(
                      (g) => !genres.includes(g) && (q === "" || g.toLowerCase().includes(q))
                    );
                    if (!suggestions.length) return null;
                    return createPortal(
                      <ul
                        style={{ position: "fixed", top: rect.bottom + 6, left: rect.left, width: rect.width, zIndex: 9999 }}
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-2xl"
                      >
                        <div className="max-h-52 overflow-y-auto">
                          {suggestions.map((g) => (
                            <li key={g}>
                              <button
                                type="button"
                                onMouseDown={(e) => { e.preventDefault(); addGenre(g); }}
                                className="w-full px-3 py-2.5 text-left text-sm text-foreground hover:bg-success/10 hover:text-success transition-colors flex items-center gap-2 group"
                              >
                                <span className="w-1.5 h-1.5 rounded-full bg-success/40 group-hover:bg-success transition-colors flex-shrink-0" />
                                {g}
                              </button>
                            </li>
                          ))}
                        </div>
                      </ul>,
                      document.body
                    );
                  })()}
                />
              </FormField>

              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.description")}</FormLabel>
                  <LockButton locked={!!lockedFields.description} onToggle={() => toggleLock("description")} disabled={isPending} />
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
            {Object.values(lockedFields).some(Boolean) && (
              <p className="text-xs text-amber-500 flex items-center gap-1.5">
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
                className="px-4 py-1.5 rounded-lg bg-primary text-white text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
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

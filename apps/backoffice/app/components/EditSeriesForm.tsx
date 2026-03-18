"use client";

import { useState, useTransition, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { useRouter } from "next/navigation";
import { FormField, FormLabel, FormInput } from "./ui/Form";

interface EditSeriesFormProps {
  libraryId: string;
  seriesName: string;
  currentAuthors: string[];
  currentPublishers: string[];
  currentBookAuthor: string | null;
  currentBookLanguage: string | null;
  currentDescription: string | null;
  currentStartYear: number | null;
}

export function EditSeriesForm({
  libraryId,
  seriesName,
  currentAuthors,
  currentPublishers,
  currentBookAuthor,
  currentBookLanguage,
  currentDescription,
  currentStartYear,
}: EditSeriesFormProps) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Champs propres à la série
  const [newName, setNewName] = useState(seriesName === "unclassified" ? "" : seriesName);
  const [authors, setAuthors] = useState<string[]>(currentAuthors);
  const [authorInput, setAuthorInput] = useState("");
  const [authorInputEl, setAuthorInputEl] = useState<HTMLInputElement | null>(null);
  const [publishers, setPublishers] = useState<string[]>(currentPublishers);
  const [publisherInput, setPublisherInput] = useState("");
  const [publisherInputEl, setPublisherInputEl] = useState<HTMLInputElement | null>(null);
  const [description, setDescription] = useState(currentDescription ?? "");
  const [startYear, setStartYear] = useState(currentStartYear?.toString() ?? "");

  // Propagation aux livres — opt-in via bouton
  const [bookAuthor, setBookAuthor] = useState(currentBookAuthor ?? "");
  const [bookLanguage, setBookLanguage] = useState(currentBookLanguage ?? "");
  const [showApplyToBooks, setShowApplyToBooks] = useState(false);

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
    setPublishers(currentPublishers);
    setPublisherInput("");
    setDescription(currentDescription ?? "");
    setStartYear(currentStartYear?.toString() ?? "");
    setShowApplyToBooks(false);
    setBookAuthor(currentBookAuthor ?? "");
    setBookLanguage(currentBookLanguage ?? "");
    setError(null);
    setIsOpen(false);
  }, [seriesName, currentAuthors, currentPublishers, currentDescription, currentStartYear, currentBookAuthor, currentBookLanguage]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isPending) handleClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, isPending, handleClose]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim() && seriesName !== "unclassified") return;
    setError(null);

    const finalAuthors = authorInput.trim()
      ? [...new Set([...authors, authorInput.trim()])]
      : authors;

    const finalPublishers = publisherInput.trim()
      ? [...new Set([...publishers, publisherInput.trim()])]
      : publishers;

    startTransition(async () => {
      try {
        const effectiveName = newName.trim() || "unclassified";
        const body: Record<string, unknown> = {
          new_name: effectiveName,
          authors: finalAuthors,
          publishers: finalPublishers,
          description: description.trim() || null,
          start_year: startYear.trim() ? parseInt(startYear.trim(), 10) : null,
        };
        if (showApplyToBooks) {
          body.author = bookAuthor.trim() || null;
          body.language = bookLanguage.trim() || null;
        }

        const res = await fetch(
          `/api/libraries/${libraryId}/series/${encodeURIComponent(seriesName)}`,
          {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
          }
        );
        if (!res.ok) {
          const data = await res.json();
          setError(data.error ?? "Erreur lors de la sauvegarde");
          return;
        }
        setIsOpen(false);

        if (effectiveName !== seriesName) {
          router.push(`/libraries/${libraryId}/series/${encodeURIComponent(effectiveName)}` as any);
        } else {
          router.refresh();
        }
      } catch {
        setError("Erreur réseau");
      }
    });
  };

  const modal = isOpen ? createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50"
        onClick={() => !isPending && handleClose()}
      />

      {/* Modal */}
      <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
        <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-2xl max-h-[90vh] overflow-y-auto animate-in fade-in zoom-in-95 duration-200">
          {/* Header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30 sticky top-0 z-10">
            <h3 className="font-semibold text-foreground">Modifier la série</h3>
            <button
              type="button"
              onClick={handleClose}
              disabled={isPending}
              className="text-muted-foreground hover:text-foreground transition-colors p-1 hover:bg-accent rounded"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* Body */}
          <form onSubmit={handleSubmit} className="p-5 space-y-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <FormField>
                <FormLabel required>Nom</FormLabel>
                <FormInput
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  disabled={isPending}
                  placeholder="Nom de la série"
                />
              </FormField>

              <FormField>
                <FormLabel>Année de début</FormLabel>
                <FormInput
                  type="number"
                  min="1900"
                  max="2100"
                  value={startYear}
                  onChange={(e) => setStartYear(e.target.value)}
                  disabled={isPending}
                  placeholder="ex : 1990"
                />
              </FormField>

              {/* Auteurs — multi-valeur */}
              <FormField className="sm:col-span-2">
                <FormLabel>Auteur(s)</FormLabel>
                <div className="space-y-2">
                  {authors.length > 0 && (
                    <div className="flex flex-wrap gap-1.5">
                      {authors.map((a, i) => (
                        <span
                          key={i}
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-primary/10 text-primary text-xs font-medium"
                        >
                          {a}
                          <button
                            type="button"
                            onClick={() => removeAuthor(i)}
                            disabled={isPending}
                            className="hover:text-destructive transition-colors ml-0.5"
                            aria-label={`Supprimer ${a}`}
                          >
                            ×
                          </button>
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="flex gap-2">
                    <input
                      ref={setAuthorInputEl}
                      value={authorInput}
                      onChange={(e) => setAuthorInput(e.target.value)}
                      onKeyDown={handleAuthorKeyDown}
                      disabled={isPending}
                      placeholder="Ajouter un auteur (Entrée pour valider)"
                      className="flex h-10 flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                    />
                    <button
                      type="button"
                      onClick={addAuthor}
                      disabled={isPending || !authorInput.trim()}
                      className="px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
                    >
                      +
                    </button>
                    <button
                      type="button"
                      onClick={() => setShowApplyToBooks(!showApplyToBooks)}
                      disabled={isPending}
                      className={`px-3 py-1.5 rounded-lg border text-sm font-medium transition-colors ${
                        showApplyToBooks
                          ? "border-primary bg-primary/10 text-primary"
                          : "border-border bg-card text-muted-foreground hover:text-foreground"
                      }`}
                      title="Appliquer auteur et langue à tous les livres de la série"
                    >
                      → livres
                    </button>
                  </div>
                </div>
              </FormField>

              {showApplyToBooks && (
                <div className="sm:col-span-2 grid grid-cols-1 sm:grid-cols-2 gap-3 pl-4 border-l-2 border-primary/30">
                  <FormField>
                    <FormLabel>Auteur (livres)</FormLabel>
                    <FormInput
                      value={bookAuthor}
                      onChange={(e) => setBookAuthor(e.target.value)}
                      disabled={isPending}
                      placeholder="Écrase le champ auteur de chaque livre"
                    />
                  </FormField>
                  <FormField>
                    <FormLabel>Langue (livres)</FormLabel>
                    <FormInput
                      value={bookLanguage}
                      onChange={(e) => setBookLanguage(e.target.value)}
                      disabled={isPending}
                      placeholder="ex : fr, en, jp"
                    />
                  </FormField>
                </div>
              )}

              {/* Éditeurs — multi-valeur */}
              <FormField className="sm:col-span-2">
                <FormLabel>Éditeur(s)</FormLabel>
                <div className="space-y-2">
                  {publishers.length > 0 && (
                    <div className="flex flex-wrap gap-1.5">
                      {publishers.map((p, i) => (
                        <span
                          key={i}
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-secondary/50 text-secondary-foreground text-xs font-medium"
                        >
                          {p}
                          <button
                            type="button"
                            onClick={() => removePublisher(i)}
                            disabled={isPending}
                            className="hover:text-destructive transition-colors ml-0.5"
                            aria-label={`Supprimer ${p}`}
                          >
                            ×
                          </button>
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="flex gap-2">
                    <input
                      ref={setPublisherInputEl}
                      value={publisherInput}
                      onChange={(e) => setPublisherInput(e.target.value)}
                      onKeyDown={handlePublisherKeyDown}
                      disabled={isPending}
                      placeholder="Ajouter un éditeur (Entrée pour valider)"
                      className="flex h-10 flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                    />
                    <button
                      type="button"
                      onClick={addPublisher}
                      disabled={isPending || !publisherInput.trim()}
                      className="px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
                    >
                      +
                    </button>
                  </div>
                </div>
              </FormField>

              <FormField className="sm:col-span-2">
                <FormLabel>Description</FormLabel>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  disabled={isPending}
                  rows={3}
                  placeholder="Synopsis ou description de la série…"
                  className="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-none"
                />
              </FormField>
            </div>

            {error && <p className="text-xs text-destructive">{error}</p>}

            {/* Footer */}
            <div className="flex items-center justify-end gap-2 pt-2 border-t border-border/50">
              <button
                type="button"
                onClick={handleClose}
                disabled={isPending}
                className="px-4 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                Annuler
              </button>
              <button
                type="submit"
                disabled={isPending || (!newName.trim() && seriesName !== "unclassified")}
                className="px-4 py-1.5 rounded-lg bg-primary text-white text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isPending ? "Sauvegarde…" : "Sauvegarder"}
              </button>
            </div>
          </form>
        </div>
      </div>
    </>,
    document.body
  ) : null;

  return (
    <>
      <button
        onClick={() => setIsOpen(true)}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
      >
        <span>✏️</span> Modifier la série
      </button>
      {modal}
    </>
  );
}

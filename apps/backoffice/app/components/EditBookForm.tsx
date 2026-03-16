"use client";

import { useState, useTransition } from "react";
import { useRouter } from "next/navigation";
import { BookDto } from "@/lib/api";
import { FormField, FormLabel, FormInput } from "./ui/Form";

interface EditBookFormProps {
  book: BookDto;
}

export function EditBookForm({ book }: EditBookFormProps) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [isEditing, setIsEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [title, setTitle] = useState(book.title);
  const [authors, setAuthors] = useState<string[]>(book.authors ?? []);
  const [authorInput, setAuthorInput] = useState("");
  const [authorInputEl, setAuthorInputEl] = useState<HTMLInputElement | null>(null);
  const [series, setSeries] = useState(book.series ?? "");
  const [volume, setVolume] = useState(book.volume?.toString() ?? "");
  const [language, setLanguage] = useState(book.language ?? "");

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

  const handleCancel = () => {
    setTitle(book.title);
    setAuthors(book.authors ?? []);
    setAuthorInput("");
    setSeries(book.series ?? "");
    setVolume(book.volume?.toString() ?? "");
    setLanguage(book.language ?? "");
    setError(null);
    setIsEditing(false);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    setError(null);

    const finalAuthors = authorInput.trim()
      ? [...new Set([...authors, authorInput.trim()])]
      : authors;

    startTransition(async () => {
      try {
        const res = await fetch(`/api/books/${book.id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            title: title.trim(),
            author: finalAuthors[0] ?? null,
            authors: finalAuthors,
            series: series.trim() || null,
            volume: volume.trim() ? parseInt(volume.trim(), 10) : null,
            language: language.trim() || null,
          }),
        });
        if (!res.ok) {
          const data = await res.json();
          setError(data.error ?? "Erreur lors de la sauvegarde");
          return;
        }
        setIsEditing(false);
        router.refresh();
      } catch {
        setError("Erreur réseau");
      }
    });
  };

  if (!isEditing) {
    return (
      <button
        onClick={() => setIsEditing(true)}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
      >
        <span>✏️</span> Modifier
      </button>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="mt-4 p-4 border border-border rounded-xl bg-muted/30 space-y-4">
      <h3 className="text-sm font-semibold text-foreground">Modifier les métadonnées</h3>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <FormField className="sm:col-span-2">
          <FormLabel required>Titre</FormLabel>
          <FormInput
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            disabled={isPending}
            placeholder="Titre du livre"
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
            </div>
          </div>
        </FormField>

        <FormField>
          <FormLabel>Langue</FormLabel>
          <FormInput
            value={language}
            onChange={(e) => setLanguage(e.target.value)}
            disabled={isPending}
            placeholder="ex : fr, en, jp"
          />
        </FormField>

        <FormField>
          <FormLabel>Série</FormLabel>
          <FormInput
            value={series}
            onChange={(e) => setSeries(e.target.value)}
            disabled={isPending}
            placeholder="Nom de la série"
          />
        </FormField>

        <FormField>
          <FormLabel>Volume</FormLabel>
          <FormInput
            type="number"
            min="1"
            value={volume}
            onChange={(e) => setVolume(e.target.value)}
            disabled={isPending}
            placeholder="Numéro de volume"
          />
        </FormField>
      </div>

      {error && (
        <p className="text-xs text-destructive">{error}</p>
      )}

      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={isPending || !title.trim()}
          className="px-4 py-1.5 rounded-lg bg-primary text-white text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {isPending ? "Sauvegarde…" : "Sauvegarder"}
        </button>
        <button
          type="button"
          onClick={handleCancel}
          disabled={isPending}
          className="px-4 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
        >
          Annuler
        </button>
      </div>
    </form>
  );
}

"use client";

import { useState, useTransition, useEffect, useCallback } from "react";
import { Modal } from "./ui/Modal";
import { useRouter } from "next/navigation";
import { BookDto } from "@/lib/api";
import { FormField, FormLabel, FormInput } from "./ui/Form";
import { useTranslation } from "../../lib/i18n/context";

function LockButton({
  locked,
  onToggle,
  disabled,
}: {
  locked: boolean;
  onToggle: () => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onToggle}
      disabled={disabled}
      className={`p-1 rounded transition-colors ${
        locked
          ? "text-amber-500 hover:text-amber-600"
          : "text-muted-foreground/40 hover:text-muted-foreground"
      }`}
      title={locked ? t("editBook.lockedField") : t("editBook.clickToLock")}
    >
      {locked ? (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
        </svg>
      ) : (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 11V7a4 4 0 118 0m-4 8v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2z" />
        </svg>
      )}
    </button>
  );
}

interface EditBookFormProps {
  book: BookDto;
}

export function EditBookForm({ book }: EditBookFormProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [title, setTitle] = useState(book.title);
  const [authors, setAuthors] = useState<string[]>(book.authors ?? []);
  const [authorInput, setAuthorInput] = useState("");
  const [authorInputEl, setAuthorInputEl] = useState<HTMLInputElement | null>(null);
  const [series, setSeries] = useState(book.series ?? "");
  const [volume, setVolume] = useState(book.volume?.toString() ?? "");
  const [language, setLanguage] = useState(book.language ?? "");
  const [summary, setSummary] = useState(book.summary ?? "");
  const [isbn, setIsbn] = useState(book.isbn ?? "");
  const [publishDate, setPublishDate] = useState(book.publish_date ?? "");
  const [lockedFields, setLockedFields] = useState<Record<string, boolean>>(book.locked_fields ?? {});

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

  const handleClose = useCallback(() => {
    setTitle(book.title);
    setAuthors(book.authors ?? []);
    setAuthorInput("");
    setSeries(book.series ?? "");
    setVolume(book.volume?.toString() ?? "");
    setLanguage(book.language ?? "");
    setSummary(book.summary ?? "");
    setIsbn(book.isbn ?? "");
    setPublishDate(book.publish_date ?? "");
    setLockedFields(book.locked_fields ?? {});
    setError(null);
    setIsOpen(false);
  }, [book]);

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
            summary: summary.trim() || null,
            isbn: isbn.trim() || null,
            publish_date: publishDate.trim() || null,
            locked_fields: lockedFields,
          }),
        });
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
    <Modal isOpen={isOpen} onClose={handleClose} title={t("editBook.editMetadata")} disableClose={isPending}>
      <form onSubmit={handleSubmit} className="p-5 space-y-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel required>{t("editBook.title")}</FormLabel>
                  <LockButton locked={!!lockedFields.title} onToggle={() => toggleLock("title")} disabled={isPending} />
                </div>
                <FormInput
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.titlePlaceholder")}
                />
              </FormField>

              {/* Auteurs — multi-valeur */}
              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.authors")}</FormLabel>
                  <LockButton locked={!!lockedFields.authors} onToggle={() => toggleLock("authors")} disabled={isPending} />
                </div>
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
                            aria-label={t("editBook.removeAuthor", { name: a })}
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
                      placeholder={t("editBook.addAuthor")}
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
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.language")}</FormLabel>
                  <LockButton locked={!!lockedFields.language} onToggle={() => toggleLock("language")} disabled={isPending} />
                </div>
                <FormInput
                  value={language}
                  onChange={(e) => setLanguage(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.languagePlaceholder")}
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.series")}</FormLabel>
                  <LockButton locked={!!lockedFields.series} onToggle={() => toggleLock("series")} disabled={isPending} />
                </div>
                <FormInput
                  value={series}
                  onChange={(e) => setSeries(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.seriesPlaceholder")}
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.volume")}</FormLabel>
                  <LockButton locked={!!lockedFields.volume} onToggle={() => toggleLock("volume")} disabled={isPending} />
                </div>
                <FormInput
                  type="number"
                  min="1"
                  value={volume}
                  onChange={(e) => setVolume(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.volumePlaceholder")}
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.isbn")}</FormLabel>
                  <LockButton locked={!!lockedFields.isbn} onToggle={() => toggleLock("isbn")} disabled={isPending} />
                </div>
                <FormInput
                  value={isbn}
                  onChange={(e) => setIsbn(e.target.value)}
                  disabled={isPending}
                  placeholder="ISBN"
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.publishDate")}</FormLabel>
                  <LockButton locked={!!lockedFields.publish_date} onToggle={() => toggleLock("publish_date")} disabled={isPending} />
                </div>
                <FormInput
                  value={publishDate}
                  onChange={(e) => setPublishDate(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.publishDatePlaceholder")}
                />
              </FormField>

              <FormField className="sm:col-span-2">
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.description")}</FormLabel>
                  <LockButton locked={!!lockedFields.summary} onToggle={() => toggleLock("summary")} disabled={isPending} />
                </div>
                <textarea
                  value={summary}
                  onChange={(e) => setSummary(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.descriptionPlaceholder")}
                  rows={4}
                  className="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-y"
                />
              </FormField>
            </div>

            {/* Lock legend */}
            {Object.values(lockedFields).some(Boolean) && (
              <p className="text-xs text-amber-500 flex items-center gap-1.5">
                <svg className="w-3.5 h-3.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                </svg>
                {t("editBook.lockedFieldsNote")}
              </p>
            )}

            {error && (
              <p className="text-xs text-destructive">{error}</p>
            )}

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
                disabled={isPending || !title.trim()}
                className="px-4 py-1.5 rounded-lg bg-primary text-white text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isPending ? t("editBook.savingLabel") : t("editBook.saveLabel")}
              </button>
            </div>
          </form>
    </Modal>
  );

  return (
    <>
      <button
        onClick={() => setIsOpen(true)}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
      >
        <span>✏️</span> {t("editBook.editMetadata")}
      </button>
      {modal}
    </>
  );
}

"use client";

import { useState, useTransition, useEffect, useCallback, useRef } from "react";
import { Modal } from "./ui/Modal";
import { useRouter } from "next/navigation";
import type { BookDetailsDto, BookItemDto } from "@/lib/api";
import { FormField, FormLabel, FormInput } from "./ui/Form";
import { Icon, TagInput } from "./ui";
import { LockButton } from "./LockButton";
import { pickLockableFields } from "@/lib/metadata/locks";
import { browserApi, browserErrorMessage, type UpdateBookRequest } from "@/lib/api/browser";
import { ApiError } from "@/lib/api/errors";
import { useLockableFields } from "./MetadataFieldsProvider";
import { useTranslation } from "@/lib/i18n/context";

interface EditBookFormProps {
  /**
   * A list item (`BookItem`) lacks the ISBN and the locked fields; the form
   * then loads the detail before allowing a save, since `PATCH /books/{id}`
   * replaces every field (saving a list item would wipe both).
   */
  book: BookItemDto | BookDetailsDto;
  children?: (open: () => void) => React.ReactNode;
}

function isBookDetails(book: BookItemDto | BookDetailsDto): book is BookDetailsDto {
  return "isbn" in book && "file_parse_status" in book;
}

export function EditBookForm({ book: initialBook, children }: EditBookFormProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // A detail passed as prop is always used as-is, so a `router.refresh()`
  // (after a save, or a change made elsewhere) is picked up. A list item has
  // no ISBN / locked fields: the detail is fetched fresh on every open, never
  // kept from a previous open, so the form never writes back stale values.
  const propDetails = isBookDetails(initialBook) ? initialBook : null;
  const [fetchedDetails, setFetchedDetails] = useState<BookDetailsDto | null>(null);
  const details = propDetails ?? fetchedDetails;
  const [loadingDetails, setLoadingDetails] = useState(false);
  const loadRequest = useRef(0);
  const book = details ?? initialBook;
  const bookIsbn = details?.isbn ?? null;
  const bookLockedFields = details?.locked_fields;

  const [title, setTitle] = useState(book.title);
  const [authors, setAuthors] = useState<string[]>(book.authors ?? []);
  const [authorInput, setAuthorInput] = useState("");
  const [authorInputEl, setAuthorInputEl] = useState<HTMLInputElement | null>(null);
  const [series, setSeries] = useState(book.series ?? "");
  const [volume, setVolume] = useState(book.volume?.toString() ?? "");
  const [language, setLanguage] = useState(book.language ?? "");
  const [summary, setSummary] = useState(book.summary ?? "");
  const [isbn, setIsbn] = useState(bookIsbn ?? "");
  const [publishDate, setPublishDate] = useState(book.publish_date ?? "");
  // The raw map is kept as-is; the catalog decides which keys are still
  // meaningful for the lock UI and the PATCH payload.
  const lockableFields = useLockableFields("books");
  const [lockedFields, setLockedFields] = useState<Record<string, boolean>>(
    () => bookLockedFields ?? {},
  );
  const visibleLockedFields = pickLockableFields(lockedFields, lockableFields);

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

  const resetFrom = useCallback((source: BookItemDto | BookDetailsDto) => {
    const sourceDetails = isBookDetails(source) ? source : null;
    setTitle(source.title);
    setAuthors(source.authors ?? []);
    setAuthorInput("");
    setSeries(source.series ?? "");
    setVolume(source.volume?.toString() ?? "");
    setLanguage(source.language ?? "");
    setSummary(source.summary ?? "");
    setIsbn(sourceDetails?.isbn ?? "");
    setPublishDate(source.publish_date ?? "");
    setLockedFields(sourceDetails?.locked_fields ?? {});
  }, []);

  const handleClose = useCallback(() => {
    loadRequest.current += 1; // ignore a detail load still in flight
    setLoadingDetails(false);
    resetFrom(book);
    setError(null);
    setIsOpen(false);
  }, [book, resetFrom]);

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
    if (!title.trim() || !details) return;
    setError(null);

    const finalAuthors = authorInput.trim()
      ? [...new Set([...authors, authorInput.trim()])]
      : authors;

    startTransition(async () => {
      try {
        const body: UpdateBookRequest = {
          title: title.trim(),
          author: finalAuthors[0] ?? null,
          authors: finalAuthors,
          series: series.trim() || null,
          volume: volume.trim() ? parseInt(volume.trim(), 10) : null,
          language: language.trim() || null,
          summary: summary.trim() || null,
          isbn: isbn.trim() || null,
          publish_date: publishDate.trim() || null,
          locked_fields: visibleLockedFields,
        };
        await browserApi.books.update(book.id, body);
        setFetchedDetails(null);
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
    <Modal isOpen={isOpen} onClose={handleClose} title={t("editBook.editMetadata")} disableClose={isPending}>
      <form onSubmit={handleSubmit} className="p-5 space-y-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <FormField className="sm:col-span-2">
                <FormLabel required>{t("editBook.title")}</FormLabel>
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
                />
              </FormField>

              <FormField>
                <div className="flex items-center gap-1">
                  <FormLabel>{t("editBook.language")}</FormLabel>
                  <LockButton locked={!!visibleLockedFields.language} onToggle={() => toggleLock("language")} disabled={isPending} />
                </div>
                <FormInput
                  value={language}
                  onChange={(e) => setLanguage(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.languagePlaceholder")}
                />
              </FormField>

              <FormField>
                <FormLabel>{t("editBook.series")}</FormLabel>
                <FormInput
                  value={series}
                  onChange={(e) => setSeries(e.target.value)}
                  disabled={isPending}
                  placeholder={t("editBook.seriesPlaceholder")}
                />
              </FormField>

              <FormField>
                <FormLabel>{t("editBook.volume")}</FormLabel>
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
                  <LockButton locked={!!visibleLockedFields.isbn} onToggle={() => toggleLock("isbn")} disabled={isPending} />
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
                  <LockButton locked={!!visibleLockedFields.publish_date} onToggle={() => toggleLock("publish_date")} disabled={isPending} />
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
                  <LockButton locked={!!visibleLockedFields.summary} onToggle={() => toggleLock("summary")} disabled={isPending} />
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
            {Object.values(visibleLockedFields).some(Boolean) && (
              <p className="text-xs text-warning flex items-center gap-1.5">
                <Icon name="lock" size="sm" className="!w-3.5 !h-3.5 shrink-0" />
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
                disabled={isPending || loadingDetails || !details || !title.trim()}
                className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isPending ? t("editBook.savingLabel") : t("editBook.saveLabel")}
              </button>
            </div>
          </form>
    </Modal>
  );

  const open = () => {
    setError(null);
    setIsOpen(true);
    if (propDetails) {
      // Start from the current prop, not from the values of a previous open.
      resetFrom(propDetails);
      return;
    }
    const request = ++loadRequest.current;
    setFetchedDetails(null);
    resetFrom(initialBook);
    setLoadingDetails(true);
    browserApi.books
      .detail(initialBook.id)
      .then((loaded) => {
        if (request !== loadRequest.current) return;
        setFetchedDetails(loaded);
        resetFrom(loaded);
      })
      .catch((err) => {
        if (request === loadRequest.current) setError(browserErrorMessage(err, t("editBook.saveError")));
      })
      .finally(() => {
        if (request === loadRequest.current) setLoadingDetails(false);
      });
  };

  return (
    <>
      {children ? (
        children(open)
      ) : (
        <button
          onClick={open}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors"
        >
          <span>✏️</span> {t("editBook.editMetadata")}
        </button>
      )}
      {modal}
    </>
  );
}

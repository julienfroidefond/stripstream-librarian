"use client";

import { FormEvent, KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useFormStatus } from "react-dom";

import { Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

import { JobLibrary, LibraryInputs, LibraryMultiSelect } from "./LibraryMultiSelect";
import { JOB_ACTION_GROUPS, JobAction, TONE_BADGE } from "./jobActionDefinitions";

function normalize(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase();
}

interface JobActionsPaletteProps {
  onClose: () => void;
  actions: JobAction[];
  libraries: JobLibrary[];
  selectedLibraries: string[] | null;
  onSelectLibraries: (value: string[] | null) => void;
}

export function JobActionsPalette({
  onClose,
  actions,
  libraries,
  selectedLibraries,
  onSelectLibraries,
}: JobActionsPaletteProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [confirmAction, setConfirmAction] = useState<JobAction | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const sections = useMemo(() => {
    const needle = normalize(query.trim());
    return JOB_ACTION_GROUPS.map((group) => ({
      ...group,
      actions: actions.filter((action) => {
        if (action.group !== group.id) return false;
        if (!needle) return true;
        const haystack = normalize(
          `${t(action.labelKey)} ${t(action.descriptionKey)} ${t(group.labelKey)}`
        );
        return haystack.includes(needle);
      }),
    })).filter((group) => group.actions.length > 0);
  }, [actions, query, t]);

  const flatActions = useMemo(() => sections.flatMap((section) => section.actions), [sections]);
  const highlightedIndex = Math.min(activeIndex, Math.max(flatActions.length - 1, 0));

  useEffect(() => {
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    function handleEscape(event: globalThis.KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      if (confirmAction) {
        setConfirmAction(null);
        requestAnimationFrame(() => inputRef.current?.focus());
      } else {
        onClose();
      }
    }

    document.addEventListener("keydown", handleEscape);
    return () => {
      document.body.style.overflow = previousOverflow;
      document.removeEventListener("keydown", handleEscape);
    };
  }, [confirmAction, onClose]);

  useEffect(() => {
    if (!confirmAction) return;
    const frame = requestAnimationFrame(() => confirmButtonRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [confirmAction]);

  useEffect(() => {
    const action = flatActions[highlightedIndex];
    if (!action) return;
    const element = itemRefs.current[action.id];
    if (element && typeof element.scrollIntoView === "function") {
      element.scrollIntoView({ block: "nearest" });
    }
  }, [highlightedIndex, flatActions]);

  function isBlocked(action: JobAction): boolean {
    return (
      (action.usesLibrary && selectedLibraries === null) ||
      Boolean(action.requiresLibrary && (!selectedLibraries || selectedLibraries.length === 0))
    );
  }

  function checkLaunchable(action: JobAction): boolean {
    if (action.usesLibrary && selectedLibraries === null) {
      setNotice(t("jobs.noLibraryWarning"));
      return false;
    }
    if (action.requiresLibrary && (!selectedLibraries || selectedLibraries.length === 0)) {
      setNotice(t("jobs.requiresLibrary"));
      return false;
    }
    setNotice(null);
    return true;
  }

  function handleActionClick(event: FormEvent<HTMLButtonElement>, action: JobAction) {
    if (!checkLaunchable(action)) {
      event.preventDefault();
      return;
    }
    if (action.confirm) {
      event.preventDefault();
      setConfirmAction(action);
    }
  }

  function handleSearchKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => (flatActions.length ? (index + 1) % flatActions.length : 0));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) =>
        flatActions.length ? (index - 1 + flatActions.length) % flatActions.length : 0
      );
      return;
    }
    if (event.key !== "Enter") return;
    event.preventDefault();
    const action = flatActions[highlightedIndex];
    if (!action || !checkLaunchable(action)) return;
    if (action.confirm) {
      setConfirmAction(action);
      return;
    }
    itemRefs.current[action.id]?.click();
  }

  function handlePanelKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Tab") return;
    const focusables = panelRef.current?.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    );
    if (!focusables || focusables.length === 0) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  const libraryBlocked = confirmAction ? isBlocked(confirmAction) : false;

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-end justify-center bg-black/40 backdrop-blur-sm animate-fade-in sm:items-start sm:p-4 sm:pt-[10vh]"
      onClick={onClose}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("jobs.allActions")}
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={handlePanelKeyDown}
        className="flex max-h-[88vh] w-full flex-col overflow-hidden rounded-t-2xl border border-border/60 bg-popover shadow-elevation-2 animate-scale-in sm:max-w-xl sm:rounded-xl"
      >
        <form className="flex min-h-0 flex-1 flex-col">
          <LibraryInputs value={selectedLibraries} />

          {confirmAction ? (
            <>
              <div className="flex items-start gap-3 border-b border-border/50 px-4 py-3">
                <span
                  className={`grid h-10 w-10 shrink-0 place-items-center rounded-lg ${TONE_BADGE[confirmAction.tone]}`}
                >
                  <Icon name={confirmAction.icon} size="md" mono />
                </span>
                <div className="min-w-0 flex-1">
                  <h2 className="text-sm font-semibold text-foreground">{t("jobs.confirmActionTitle")}</h2>
                  <p className="truncate text-xs text-muted-foreground">{t(confirmAction.labelKey)}</p>
                </div>
                <button
                  type="button"
                  onClick={() => setConfirmAction(null)}
                  aria-label={t("common.close")}
                  className="cursor-pointer rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  <Icon name="x" size="sm" mono />
                </button>
              </div>

              <div className="px-4 py-4">
                <p className="text-sm leading-relaxed text-foreground">{t(confirmAction.descriptionKey)}</p>
                <p className="mt-3 rounded-lg border border-destructive/20 bg-destructive/5 px-3 py-2 text-xs text-destructive">
                  {t("jobs.confirmActionWarning")}
                </p>
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  <span className="text-xs text-muted-foreground">{t("jobs.libraryLabel")}</span>
                  <LibraryMultiSelect
                    libraries={libraries}
                    value={selectedLibraries}
                    onChange={(value) => {
                      setNotice(null);
                      onSelectLibraries(value);
                    }}
                  />
                  {libraryBlocked && (
                    <span className="text-xs text-destructive">
                      {confirmAction.requiresLibrary ? t("jobs.requiresLibrary") : t("jobs.noLibraryWarning")}
                    </span>
                  )}
                </div>
              </div>

              <div className="flex items-center justify-end gap-2 border-t border-border/50 px-4 py-3">
                <button
                  type="button"
                  onClick={() => setConfirmAction(null)}
                  className="h-9 cursor-pointer rounded-lg px-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  {t("common.cancel")}
                </button>
                <button
                  ref={confirmButtonRef}
                  type="submit"
                  formAction={confirmAction.action}
                  disabled={libraryBlocked}
                  onClick={(event) => {
                    if (!checkLaunchable(confirmAction)) event.preventDefault();
                  }}
                  className="inline-flex h-9 cursor-pointer items-center gap-2 rounded-lg bg-destructive px-3 text-sm font-medium text-destructive-foreground transition-colors hover:bg-destructive/90 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <Icon name="play" size="sm" mono />
                  {t("jobs.confirmAction")}
                </button>
              </div>
              <PaletteStatus />
            </>
          ) : (
            <>
              <div className="flex items-center gap-2 border-b border-border/50 px-3 py-2.5">
                <Icon name="search" size="sm" mono className="text-muted-foreground" />
                <input
                  ref={inputRef}
                  value={query}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setActiveIndex(0);
                  }}
                  onKeyDown={handleSearchKeyDown}
                  placeholder={t("jobs.searchActions")}
                  aria-label={t("jobs.searchActions")}
                  autoComplete="off"
                  spellCheck={false}
                  className="h-8 min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
                />
                <button
                  type="button"
                  onClick={onClose}
                  aria-label={t("common.close")}
                  className="cursor-pointer rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  <Icon name="x" size="sm" mono />
                </button>
              </div>

              <div className="flex flex-wrap items-center gap-2 border-b border-border/50 px-3 py-2">
                <span className="text-xs text-muted-foreground">{t("jobs.libraryLabel")}</span>
                <LibraryMultiSelect
                  libraries={libraries}
                  value={selectedLibraries}
                  onChange={(value) => {
                    setNotice(null);
                    onSelectLibraries(value);
                  }}
                />
                {selectedLibraries !== null && selectedLibraries.length === 0 && (
                  <span className="text-[11px] text-muted-foreground">{t("jobs.requiresLibraryHint")}</span>
                )}
              </div>

              {notice && (
                <p role="alert" className="border-b border-destructive/20 bg-destructive/5 px-3 py-2 text-xs text-destructive">
                  {notice}
                </p>
              )}

              <div className="min-h-0 flex-1 overflow-y-auto p-2">
                {sections.length === 0 ? (
                  <p className="px-3 py-8 text-center text-sm text-muted-foreground">
                    {t("jobs.noMatchingAction")}
                  </p>
                ) : (
                  sections.map((section) => (
                    <div key={section.id} className="mb-1 last:mb-0">
                      <div className="sticky top-0 z-10 flex items-center gap-1.5 bg-popover/95 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground backdrop-blur">
                        <Icon name={section.icon} size="sm" mono className="h-3.5 w-3.5" />
                        {t(section.labelKey)}
                      </div>
                      {section.actions.map((action) => {
                        const index = flatActions.findIndex((candidate) => candidate.id === action.id);
                        const isActive = index === highlightedIndex;
                        return (
                          <button
                            key={action.id}
                            ref={(element) => {
                              itemRefs.current[action.id] = element;
                            }}
                            type="submit"
                            formAction={action.action}
                            onClick={(event) => handleActionClick(event, action)}
                            onMouseMove={() => setActiveIndex(index)}
                            aria-current={isActive ? "true" : undefined}
                            className={`flex w-full cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors ${
                              isActive ? "bg-accent" : "hover:bg-accent/60"
                            }`}
                          >
                            <span
                              className={`grid h-8 w-8 shrink-0 place-items-center rounded-lg ${TONE_BADGE[action.tone]}`}
                            >
                              <Icon name={action.icon} size="sm" mono />
                            </span>
                            <span className="min-w-0 flex-1">
                              <span className="flex items-center gap-1.5">
                                <span className="truncate text-sm font-medium text-foreground">
                                  {t(action.labelKey)}
                                </span>
                                {action.requiresLibrary && (
                                  <Icon
                                    name="lock"
                                    size="sm"
                                    mono
                                    className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
                                  />
                                )}
                                {action.confirm && (
                                  <span className="shrink-0 rounded bg-destructive/10 px-1.5 py-0.5 text-[10px] font-medium text-destructive">
                                    {t("jobs.destructiveBadge")}
                                  </span>
                                )}
                              </span>
                              <span className="block truncate text-xs text-muted-foreground">
                                {t(action.descriptionKey)}
                              </span>
                            </span>
                            <Icon
                              name="chevronRight"
                              size="sm"
                              mono
                              className="shrink-0 text-muted-foreground opacity-40"
                            />
                          </button>
                        );
                      })}
                    </div>
                  ))
                )}
              </div>

              <div className="flex items-center justify-between gap-3 border-t border-border/50 px-3 py-2 text-[11px] text-muted-foreground">
                <span>{t("jobs.paletteHint")}</span>
                <span className="hidden shrink-0 items-center gap-1 sm:flex">
                  <kbd className="rounded border border-border bg-muted/60 px-1.5 py-0.5 font-medium">⌘J</kbd>
                </span>
              </div>
              <PaletteStatus />
            </>
          )}
        </form>
      </div>
    </div>,
    document.body
  );
}

function PaletteStatus() {
  const { pending } = useFormStatus();
  const { t } = useTranslation();

  if (!pending) return null;
  return (
    <div className="flex items-center justify-center gap-2 border-t border-border/50 bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
      <Icon name="spinner" size="sm" mono className="animate-spin" />
      {t("jobs.actionPending")}
    </div>
  );
}

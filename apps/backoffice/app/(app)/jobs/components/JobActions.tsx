"use client";

import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Icon, Tooltip } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

import { JobActionsPalette } from "./JobActionsPalette";
import { JobLibrary, LibraryInputs, LibraryMultiSelect } from "./LibraryMultiSelect";
import { getJobActions, JobAction } from "./jobActionDefinitions";

interface JobActionsProps {
  libraries: JobLibrary[];
  initialSelected: string[] | null;
  readingStatus: boolean;
  prowlarr: boolean;
  ddl: boolean;
  telegram: boolean;
}

/** Left-to-right order of the one-click actions in the compact toolbar. */
const QUICK_ACTION_ORDER = [
  "rebuild",
  "metadataRefresh",
  "thumbnailsRebuild",
  "downloadDetection",
  "ddlSearch",
  "telegramSync",
];

export function JobActions({
  libraries,
  initialSelected,
  readingStatus,
  prowlarr,
  ddl,
  telegram,
}: JobActionsProps) {
  const { t } = useTranslation();
  const [selectedLibraries, setSelectedLibraries] = useState<string[] | null>(() => {
    if (initialSelected === null) return null;
    if (initialSelected.length === 0) return [];
    const known = initialSelected.filter((id) => libraries.some((library) => library.id === id));
    return known.length > 0 ? known : null;
  });
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const paletteOpenRef = useRef(false);

  const actions = useMemo(
    () => getJobActions({ readingStatus, prowlarr, ddl, telegram }),
    [readingStatus, prowlarr, ddl, telegram]
  );

  const quickActions = useMemo(
    () =>
      actions
        .filter((action) => action.quick)
        .sort((a, b) => QUICK_ACTION_ORDER.indexOf(a.id) - QUICK_ACTION_ORDER.indexOf(b.id)),
    [actions]
  );

  useEffect(() => {
    paletteOpenRef.current = paletteOpen;
  }, [paletteOpen]);

  const closePalette = useCallback(() => {
    setPaletteOpen(false);
    requestAnimationFrame(() => triggerRef.current?.focus());
  }, []);

  const openPalette = useCallback(() => {
    setNotice(null);
    setPaletteOpen(true);
  }, []);

  const changeLibraries = useCallback((value: string[] | null) => {
    setNotice(null);
    setSelectedLibraries(value);
  }, []);

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLocaleLowerCase() !== "j") return;
      event.preventDefault();
      if (paletteOpenRef.current) {
        closePalette();
      } else {
        openPalette();
      }
    }
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [closePalette, openPalette]);

  function handleQuickSubmit(event: FormEvent<HTMLFormElement>, action: JobAction) {
    if (action.usesLibrary && selectedLibraries === null) {
      event.preventDefault();
      setNotice(t("jobs.noLibraryWarning"));
      return;
    }
    if (action.requiresLibrary && (!selectedLibraries || selectedLibraries.length === 0)) {
      event.preventDefault();
      setNotice(t("jobs.requiresLibrary"));
    }
  }

  return (
    <div className="mb-6">
      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-border/60 bg-card/50 p-2">
        <LibraryMultiSelect
          libraries={libraries}
          value={selectedLibraries}
          onChange={changeLibraries}
          className="w-full min-w-0 sm:w-auto sm:max-w-xs"
        />

        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          {quickActions.map((action, index) => {
            const quickLabel = action.quickLabelKey
              ? t(action.quickLabelKey)
              : t(action.labelKey);
            return (
              <form
                key={action.id}
                action={action.action}
                onSubmit={(event) => handleQuickSubmit(event, action)}
              >
                <LibraryInputs value={selectedLibraries} />
                <Tooltip
                  label={t(action.labelKey)}
                  description={action.quickHintKey ? t(action.quickHintKey) : undefined}
                  align={index === quickActions.length - 1 ? "end" : "center"}
                >
                  <button
                    type="submit"
                    aria-label={quickLabel}
                    className="inline-flex h-9 cursor-pointer items-center gap-1.5 rounded-lg border border-border bg-background px-2.5 text-sm font-medium text-foreground shadow-sm transition-colors hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  >
                    <Icon
                      name={action.icon}
                      size="sm"
                      mono
                      className="shrink-0 text-muted-foreground"
                    />
                    <span className="hidden sm:inline">{quickLabel}</span>
                  </button>
                </Tooltip>
              </form>
            );
          })}

          <span className="mx-0.5 hidden h-5 w-px bg-border sm:block" aria-hidden="true" />

          <button
            ref={triggerRef}
            type="button"
            onClick={openPalette}
            aria-haspopup="dialog"
            aria-expanded={paletteOpen}
            className="inline-flex h-9 cursor-pointer items-center gap-2 rounded-lg bg-primary px-3 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          >
            <Icon name="jobs" size="sm" mono />
            <span className="hidden sm:inline">{t("jobs.allActions")}</span>
            <span className="sm:hidden">{t("jobs.actionsShort")}</span>
            <kbd className="hidden rounded border border-primary-foreground/30 bg-primary-foreground/10 px-1.5 py-0.5 text-[10px] font-medium lg:inline">
              ⌘J
            </kbd>
          </button>
        </div>
      </div>

      {notice && (
        <p
          role="alert"
          className="mt-2 rounded-lg border border-destructive/20 bg-destructive/5 px-3 py-2 text-xs text-destructive"
        >
          {notice}
        </p>
      )}

      {paletteOpen && (
        <JobActionsPalette
          onClose={closePalette}
          actions={actions}
          libraries={libraries}
          selectedLibraries={selectedLibraries}
          onSelectLibraries={changeLibraries}
        />
      )}
    </div>
  );
}

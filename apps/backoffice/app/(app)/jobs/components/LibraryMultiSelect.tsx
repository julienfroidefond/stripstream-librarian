"use client";

import {
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type { CSSProperties, RefObject } from "react";
import { createPortal } from "react-dom";

import { Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

export interface JobLibrary {
  id: string;
  name: string;
}

/**
 * Hidden inputs consumed by the server actions.
 * No value means "all libraries" (the UI blocks launching with no selection).
 */
export function LibraryInputs({ value }: { value: string[] | null }) {
  if (!value || value.length === 0) {
    return <input type="hidden" name="library_id" value="" />;
  }
  return (
    <>
      {value.map((id) => (
        <input key={id} type="hidden" name="library_id" value={id} />
      ))}
    </>
  );
}

interface LibraryMultiSelectProps {
  libraries: JobLibrary[];
  /** `null` = nothing picked yet, `[]` = all libraries, otherwise the selected ids. */
  value: string[] | null;
  onChange: (value: string[] | null) => void;
  buttonRef?: RefObject<HTMLButtonElement | null>;
  className?: string;
}

function subscribeToViewport(callback: () => void) {
  window.addEventListener("resize", callback);
  return () => window.removeEventListener("resize", callback);
}

function getMobileSnapshot() {
  return window.innerWidth < 640;
}

function computePanelStyle(trigger: HTMLElement): CSSProperties {
  const rect = trigger.getBoundingClientRect();
  const width = Math.min(320, window.innerWidth - 24);
  const roomBelow = window.innerHeight - rect.bottom - 24;
  const roomAbove = rect.top - 24;
  const openAbove = roomBelow < 220 && roomAbove > roomBelow;
  return {
    position: "fixed",
    width: `${width}px`,
    left: `${Math.min(Math.max(12, rect.left), window.innerWidth - width - 12)}px`,
    maxHeight: `${Math.max(180, Math.min(380, openAbove ? roomAbove : roomBelow))}px`,
    ...(openAbove
      ? { bottom: `${window.innerHeight - rect.top + 8}px` }
      : { top: `${rect.bottom + 8}px` }),
  };
}

export function LibraryMultiSelect({
  libraries,
  value,
  onChange,
  buttonRef,
  className = "",
}: LibraryMultiSelectProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [panelStyle, setPanelStyle] = useState<CSSProperties>({});
  const localButtonRef = useRef<HTMLButtonElement>(null);
  const triggerRef = buttonRef ?? localButtonRef;
  const panelRef = useRef<HTMLDivElement>(null);
  const isMobile = useSyncExternalStore(subscribeToViewport, getMobileSnapshot, () => false);

  const allSelected = value !== null && value.length === 0;

  const summary = (() => {
    if (value === null) return t("jobs.chooseLibrary");
    if (value.length === 0) return t("jobs.allLibrariesOption");
    if (value.length === 1) {
      return (
        libraries.find((library) => library.id === value[0])?.name ??
        t("jobs.librariesSelected", { count: 1 })
      );
    }
    return t("jobs.librariesSelected", { count: value.length });
  })();

  const selectedNames =
    value && value.length > 0
      ? libraries
          .filter((library) => value.includes(library.id))
          .map((library) => library.name)
      : [];

  const updatePosition = () => {
    const trigger = triggerRef.current;
    if (trigger) setPanelStyle(computePanelStyle(trigger));
  };

  function toggleOpen() {
    if (open) {
      setOpen(false);
      return;
    }
    if (!isMobile) updatePosition();
    setOpen(true);
  }

  useEffect(() => {
    if (!open || isMobile) return;
    function reposition() {
      const trigger = triggerRef.current;
      if (trigger) setPanelStyle(computePanelStyle(trigger));
    }
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [open, isMobile, triggerRef]);

  useEffect(() => {
    if (!open) return;
    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || panelRef.current?.contains(target)) return;
      setOpen(false);
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, triggerRef]);

  useEffect(() => {
    if (!open || !isMobile) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previousOverflow;
    };
  }, [open, isMobile]);

  function toggleAll() {
    if (allSelected) {
      onChange(null);
      return;
    }
    onChange([]);
    setOpen(false);
  }

  function toggleLibrary(id: string) {
    if (value === null || value.length === 0) {
      onChange([id]);
      return;
    }
    const next = value.includes(id)
      ? value.filter((selectedId) => selectedId !== id)
      : [...value, id];
    onChange(next.length > 0 ? next : null);
  }

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={toggleOpen}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`${t("jobs.libraryLabel")}: ${summary}`}
        title={selectedNames.length > 1 ? selectedNames.join(", ") : undefined}
        className={`inline-flex h-9 min-w-0 cursor-pointer items-center gap-2 rounded-lg border border-border bg-background px-2.5 text-sm text-foreground shadow-sm transition-colors hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${className}`}
      >
        <Icon name="libraries" size="sm" mono className="shrink-0 text-muted-foreground" />
        <span className="truncate">{summary}</span>
        <Icon
          name="chevronDown"
          size="sm"
          mono
          className={`ml-auto shrink-0 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open &&
        createPortal(
          <>
            {isMobile && (
              <div
                className="fixed inset-0 z-[110] bg-black/40 backdrop-blur-sm animate-fade-in"
                onClick={() => setOpen(false)}
                aria-hidden="true"
              />
            )}
            <div
              ref={panelRef}
              role="dialog"
              aria-modal={isMobile || undefined}
              aria-label={t("jobs.libraryPickerTitle")}
              style={isMobile ? undefined : panelStyle}
              className={
                isMobile
                  ? "fixed inset-x-0 bottom-0 z-[111] max-h-[80vh] overflow-y-auto rounded-t-2xl border-t border-border/60 bg-popover p-3 pb-[calc(env(safe-area-inset-bottom)+0.75rem)] shadow-elevation-2 animate-fade-in"
                  : "fixed z-[111] overflow-y-auto rounded-xl border border-border/60 bg-popover p-1.5 shadow-elevation-2 animate-fade-in"
              }
            >
              <div className="px-2 py-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                {t("jobs.libraryPickerTitle")}
              </div>

              <label className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-foreground transition-colors hover:bg-accent">
                <input
                  type="checkbox"
                  className="h-4 w-4 shrink-0 cursor-pointer accent-primary"
                  checked={allSelected}
                  onChange={toggleAll}
                />
                <span className="font-medium">{t("jobs.allLibrariesOption")}</span>
              </label>

              <div className="my-1 h-px bg-border/60" aria-hidden="true" />

              <div className="space-y-0.5">
                {libraries.map((library) => (
                  <label
                    key={library.id}
                    className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-foreground transition-colors hover:bg-accent"
                  >
                    <input
                      type="checkbox"
                      className="h-4 w-4 shrink-0 cursor-pointer accent-primary"
                      checked={value?.includes(library.id) ?? false}
                      onChange={() => toggleLibrary(library.id)}
                    />
                    <span className="truncate">{library.name}</span>
                  </label>
                ))}
              </div>

              <p className="px-2 pt-2 text-[11px] leading-snug text-muted-foreground">
                {t("jobs.multiLibraryHint")}
              </p>

              <div className="mt-2 flex items-center justify-between gap-2 border-t border-border/60 px-1 pt-2">
                <button
                  type="button"
                  onClick={() => onChange(null)}
                  disabled={value === null}
                  className="cursor-pointer rounded-md px-2 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {t("jobs.clearSelection")}
                </button>
                <button
                  type="button"
                  onClick={() => setOpen(false)}
                  className="cursor-pointer rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                >
                  {t("jobs.libraryPickerDone")}
                </button>
              </div>
            </div>
          </>,
          document.body
        )}
    </>
  );
}

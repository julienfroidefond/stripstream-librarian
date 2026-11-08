"use client";

import { createPortal } from "react-dom";
import { ReactNode, useEffect, useRef } from "react";

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled]):not([type='hidden'])",
  "select:not([disabled])",
  "[contenteditable='true']",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

const MAX_WIDTH_MAP = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
  xl: "max-w-xl",
  "2xl": "max-w-2xl",
  "3xl": "max-w-3xl",
  "4xl": "max-w-4xl",
  "5xl": "max-w-5xl",
} as const;

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  maxWidth?: keyof typeof MAX_WIDTH_MAP;
  /** Disable closing via backdrop click or Escape (e.g. while a form is submitting) */
  disableClose?: boolean;
}

export function Modal({ isOpen, onClose, title, children, footer, maxWidth = "2xl", disableClose = false }: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLElement | null>(null);
  const lastFocusedRef = useRef<HTMLElement | null>(null);

  // Mémorise l'élément qui reçoit le focus tant que la modale est fermée. Un
  // `autoFocus` déplace le focus dans le panneau pendant le commit : un effet
  // d'ouverture ne peut donc plus lire le déclencheur dans `activeElement`.
  useEffect(() => {
    if (isOpen) return;

    const remember = (event: Event) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      // L'`autoFocus` du panneau peut émettre un `focusin` avant que l'effet
      // de fermeture n'ait retiré cet écouteur : il ne doit pas remplacer le
      // déclencheur mémorisé.
      if (target.closest('[data-testid="modal-panel"]')) return;
      if (panelRef.current?.contains(target)) return;
      const focusable = target.closest<HTMLElement>(FOCUSABLE_SELECTOR);
      if (focusable) lastFocusedRef.current = focusable;
    };

    document.addEventListener("focusin", remember);
    document.addEventListener("pointerdown", remember, true);
    return () => {
      document.removeEventListener("focusin", remember);
      document.removeEventListener("pointerdown", remember, true);
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen || disableClose) return;
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleEsc);
    return () => document.removeEventListener("keydown", handleEsc);
  }, [isOpen, disableClose, onClose]);

  // Déplace le focus dans la modale, piège Tab/Shift+Tab et le restaure à la
  // fermeture. Le portail n'est monté qu'après l'ouverture : l'effet s'exécute
  // donc toujours quand le panneau existe.
  useEffect(() => {
    if (!isOpen) return;

    const panel = panelRef.current;
    if (!panel) return;

    if (!triggerRef.current) triggerRef.current = lastFocusedRef.current;
    const activeElement =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!triggerRef.current && activeElement && !panel.contains(activeElement)) {
      triggerRef.current = activeElement;
    }

    const focusableElements = () =>
      Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));

    if (!panel.contains(document.activeElement)) {
      const autoFocusTarget = panel.querySelector<HTMLElement>("[autofocus]");
      (autoFocusTarget ?? focusableElements()[0] ?? panel).focus();
    }

    const handleTab = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const elements = focusableElements();
      if (elements.length === 0) {
        event.preventDefault();
        panel.focus();
        return;
      }

      const first = elements[0];
      const last = elements[elements.length - 1];
      const active = document.activeElement;
      const outside = !panel.contains(active);

      if (event.shiftKey && (active === first || outside)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || outside)) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", handleTab);
    return () => {
      document.removeEventListener("keydown", handleTab);
      triggerRef.current?.focus();
      triggerRef.current = null;
      lastFocusedRef.current = null;
    };
  }, [isOpen]);

  if (!isOpen) return null;

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/30 backdrop-blur-sm animate-fade-in"
      onClick={() => !disableClose && onClose()}
    >
      <div
        ref={panelRef}
        data-testid="modal-panel"
        role="dialog"
        aria-modal="true"
        tabIndex={-1}
        className={`bg-card border border-border/50 rounded-xl shadow-2xl w-full ${MAX_WIDTH_MAP[maxWidth]} max-h-[90vh] overflow-y-auto animate-scale-in outline-none`}
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        {title && (
          <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30 sticky top-0 z-10">
            <h3 className="font-semibold text-foreground">{title}</h3>
            <button
              type="button"
              data-testid="modal-close"
              onClick={onClose}
              disabled={disableClose}
              className="text-muted-foreground hover:text-foreground transition-colors p-1 hover:bg-accent rounded"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        )}

        {/* Body */}
        {children}

        {/* Footer */}
        {footer && (
          <div className="px-5 py-4 border-t border-border/50 bg-muted/30 sticky bottom-0">
            {footer}
          </div>
        )}
      </div>
    </div>,
    document.body
  );
}

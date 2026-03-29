"use client";

import { createPortal } from "react-dom";
import { ReactNode } from "react";

const MAX_WIDTH_MAP = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
  xl: "max-w-xl",
  "2xl": "max-w-2xl",
  "3xl": "max-w-3xl",
} as const;

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: ReactNode;
  maxWidth?: keyof typeof MAX_WIDTH_MAP;
  /** Disable closing via backdrop click (e.g. while a form is submitting) */
  disableClose?: boolean;
}

export function Modal({ isOpen, onClose, title, children, maxWidth = "2xl", disableClose = false }: ModalProps) {
  if (!isOpen) return null;

  return createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50"
        onClick={() => !disableClose && onClose()}
      />

      {/* Container */}
      <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
        <div className={`bg-card border border-border/50 rounded-xl shadow-2xl w-full ${MAX_WIDTH_MAP[maxWidth]} max-h-[90vh] overflow-y-auto animate-in fade-in zoom-in-95 duration-200`}>
          {/* Header */}
          {title && (
            <div className="flex items-center justify-between px-5 py-4 border-b border-border/50 bg-muted/30 sticky top-0 z-10">
              <h3 className="font-semibold text-foreground">{title}</h3>
              <button
                type="button"
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
        </div>
      </div>
    </>,
    document.body
  );
}

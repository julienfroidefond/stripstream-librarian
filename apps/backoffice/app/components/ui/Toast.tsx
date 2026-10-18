"use client";

import { useSyncExternalStore, useCallback } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon";

type ToastVariant = "success" | "error" | "info";

interface ToastItem {
  id: number;
  message: string;
  variant: ToastVariant;
  /** Set while the exit animation plays, just before the toast is removed. */
  leaving?: boolean;
}

// How long a toast stays on screen, then how long its exit animation lasts.
const TOAST_DURATION = 3000;
const TOAST_EXIT = 180;

// Module-level store — no Provider needed
let toasts: ToastItem[] = [];
let nextId = 0;
const listeners = new Set<() => void>();

function notify() {
  listeners.forEach((l) => l());
}

function addToast(message: string, variant: ToastVariant = "success") {
  const id = nextId++;
  toasts = [...toasts, { id, message, variant }];
  notify();
  setTimeout(() => {
    beginRemoveToast(id);
  }, TOAST_DURATION);
}

/** Flags a toast as leaving so it plays its exit animation, then drops it. */
function beginRemoveToast(id: number) {
  if (!toasts.some((t) => t.id === id && !t.leaving)) return;
  toasts = toasts.map((t) => (t.id === id ? { ...t, leaving: true } : t));
  notify();
  setTimeout(() => {
    removeToast(id);
  }, TOAST_EXIT);
}

function removeToast(id: number) {
  toasts = toasts.filter((t) => t.id !== id);
  notify();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return toasts;
}

const EMPTY_TOASTS: ToastItem[] = [];
function getServerSnapshot(): ToastItem[] {
  return EMPTY_TOASTS;
}

export function toast(message: string, variant?: ToastVariant) {
  addToast(message, variant);
}

export function Toaster() {
  const items = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);

  if (items.length === 0) return null;

  if (typeof document === "undefined") return null;

  return createPortal(
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {items.map((t) => (
        <ToastCard key={t.id} item={t} onDismiss={() => beginRemoveToast(t.id)} />
      ))}
    </div>,
    document.body
  );
}

const variantStyles: Record<ToastVariant, string> = {
  success: "border-success/50 bg-success/10 text-success",
  error: "border-destructive/50 bg-destructive/10 text-destructive",
  info: "border-primary/50 bg-primary/10 text-primary",
};

const variantIcons: Record<ToastVariant, "check" | "warning" | "eye"> = {
  success: "check",
  error: "warning",
  info: "eye",
};

function ToastCard({ item, onDismiss }: { item: ToastItem; onDismiss: () => void }) {
  return (
    <div
      className={`flex items-center gap-3 px-4 py-3 rounded-lg border shadow-lg backdrop-blur-sm min-w-[280px] max-w-[420px] ${
        item.leaving ? "animate-slide-out-right" : "animate-slide-in-right"
      } ${variantStyles[item.variant]}`}
    >
      <Icon name={variantIcons[item.variant]} size="sm" />
      <span className="text-sm font-medium flex-1">{item.message}</span>
      <button
        onClick={onDismiss}
        className="opacity-60 hover:opacity-100 transition-opacity"
      >
        <Icon name="x" size="sm" />
      </button>
    </div>
  );
}

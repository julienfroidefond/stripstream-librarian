"use client";

import { useSyncExternalStore, useCallback } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon";

type ToastVariant = "success" | "error" | "info";

interface ToastItem {
  id: number;
  message: string;
  variant: ToastVariant;
}

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
    removeToast(id);
  }, 3000);
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
        <ToastCard key={t.id} item={t} onDismiss={() => removeToast(t.id)} />
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
      className={`flex items-center gap-3 px-4 py-3 rounded-lg border shadow-lg backdrop-blur-sm animate-slide-in-right min-w-[280px] max-w-[420px] ${variantStyles[item.variant]}`}
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

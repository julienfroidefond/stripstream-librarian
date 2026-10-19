"use client";

import { useTransition } from "react";
import { useRouter } from "next/navigation";

import { useTranslation } from "@/lib/i18n/context";
import {
  VIEW_MODE_COOKIE,
  VIEW_MODE_MAX_AGE,
  serializeViewMode,
  type ViewMode,
} from "@/lib/viewMode";
import { Icon } from "./ui";

interface ViewModeToggleProps {
  viewMode: ViewMode;
  className?: string;
}

/**
 * Bascule grille / liste des listes de livres et de séries.
 *
 * La préférence vit dans le cookie `view_mode` : le serveur la lit au rendu,
 * le bouton l'écrit puis rafraîchit le Server Component — même mécanique que
 * `LiveSearchForm` pour les filtres (l'URL serait écrasée par ces derniers).
 */
export function ViewModeToggle({ viewMode, className }: ViewModeToggleProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  const next: ViewMode = viewMode === "grid" ? "list" : "grid";
  const label = next === "list" ? t("books.display.list") : t("books.display.grid");

  const handleClick = () => {
    document.cookie = `${VIEW_MODE_COOKIE}=${serializeViewMode(next)};path=/;max-age=${VIEW_MODE_MAX_AGE};SameSite=Lax`;
    startTransition(() => router.refresh());
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={isPending}
      aria-label={label}
      aria-pressed={viewMode === "list"}
      title={label}
      className={`inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-full border-0 bg-background px-3 text-xs font-medium ring-1 ring-input transition-all duration-200 ease-out hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 active:scale-[0.98] ${className ?? ""}`}
    >
      <Icon name={next === "list" ? "list" : "grid"} size="sm" />
      <span className="hidden sm:inline">{label}</span>
    </button>
  );
}

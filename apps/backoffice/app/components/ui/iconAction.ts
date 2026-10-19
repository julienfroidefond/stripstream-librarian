/**
 * Look of the icon-only controls used by the card action clusters.
 *
 * Two shapes, one per display mode: a small dark round pill laid over a cover
 * (`cover`) and a neutral outlined square sitting on a row background
 * (`inline`). Shared by the mark-read toggle, the `⋯` menu trigger, the
 * download shortcut and the delete shortcut so a cluster stays aligned.
 *
 * `tone` carries the semantic state: neutral for menus and downloads, danger
 * for destructive actions, success for an item already marked as read.
 */

export type IconActionVariant = "cover" | "inline";
export type IconActionTone = "neutral" | "danger" | "success";

const COVER_TONES: Record<IconActionTone, string> = {
  neutral: "bg-black/65 text-white backdrop-blur-sm hover:bg-black/85",
  danger: "bg-black/65 text-red-300 backdrop-blur-sm hover:bg-destructive hover:text-white",
  success: "bg-success/90 text-success-foreground backdrop-blur-sm hover:bg-success",
};

const INLINE_TONES: Record<IconActionTone, string> = {
  neutral:
    "border border-border bg-card text-muted-foreground hover:border-primary hover:text-foreground",
  danger: "border border-destructive/40 bg-card text-destructive hover:bg-destructive/10",
  success: "border border-success/30 bg-success/10 text-success hover:bg-success/20",
};

export function iconActionClassName(
  variant: IconActionVariant,
  tone: IconActionTone = "neutral",
): string {
  const base =
    "inline-flex shrink-0 items-center justify-center transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

  return variant === "cover"
    ? `${base} h-7 w-7 rounded-full ${COVER_TONES[tone]}`
    : `${base} h-8 w-8 rounded-lg ${INLINE_TONES[tone]}`;
}

/** Spacing between the controls of a cluster, per display mode. */
export function iconActionGroupClassName(variant: IconActionVariant): string {
  return variant === "cover" ? "gap-1" : "gap-2";
}

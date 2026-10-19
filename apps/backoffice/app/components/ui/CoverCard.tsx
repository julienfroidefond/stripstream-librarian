"use client";

import Image from "next/image";
import { useState, type ReactNode } from "react";

import { GlowLayer } from "../motion/GlowLayer";

/**
 * Shared chrome for the cover-first cards used by books and series.
 *
 * The visual language mirrors the reader (`stripstream`): a flat cover framed
 * by a hairline ring, a pointer-following glow, and a scrim that slides in
 * from the bottom on hover/focus. Card-specific content (badges, overlay,
 * actions) is composed by the caller.
 */
export function CoverCard({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      // `data-cover-card` is the anchor the `⋯` menu uses to render its popin
      // *inside* the cover (grid view) instead of a viewport-anchored dropdown.
      data-cover-card=""
      className={`group relative aspect-[2/3] overflow-hidden rounded-xl bg-muted shadow-sm ring-1 ring-border/50 transition-[translate,box-shadow] duration-300 ease-out hover:-translate-y-1 hover:shadow-md hover:ring-border ${className ?? ""}`}
    >
      {children}
      <GlowLayer />
    </div>
  );
}

/**
 * Row shell for the list view: the cover-first language of `CoverCard`, laid
 * out horizontally. Keeps the shared `group` + pointer-glow contract so the
 * nested `CoverImage` hover zoom and `GlowLayer` keep working.
 */
export function CoverRow({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`group relative flex items-center gap-3 overflow-hidden rounded-xl bg-card p-2 shadow-sm ring-1 ring-border/50 transition-[background-color,box-shadow] duration-200 ease-out hover:bg-accent/40 hover:shadow-md hover:ring-border sm:gap-4 sm:p-2.5 ${className ?? ""}`}
    >
      {children}
      <GlowLayer />
    </div>
  );
}

/**
 * Cover image with skeleton, load/error states and the reader's hover zoom.
 * Renders only the visual: the surrounding card owns navigation and badges.
 */
export function CoverImage({
  src,
  alt,
  sizes = "(max-width: 640px) 50vw, (max-width: 768px) 33vw, (max-width: 1024px) 25vw, 16vw",
  dimmed = false,
  fallback = "book",
}: {
  src: string;
  alt: string;
  sizes?: string;
  dimmed?: boolean;
  fallback?: "book" | "series";
}) {
  const [isLoaded, setIsLoaded] = useState(false);
  const [hasError, setHasError] = useState(false);

  if (!src || hasError) {
    return (
      <div className="absolute inset-0 flex items-center justify-center text-muted-foreground/30">
        {fallback === "series" ? (
          <svg className="h-12 w-12" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25" />
          </svg>
        ) : (
          <svg className="h-10 w-10" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
        )}
      </div>
    );
  }

  return (
    <>
      <div
        className={`skeleton-shimmer absolute inset-0 transition-opacity duration-300 ${
          isLoaded ? "pointer-events-none opacity-0" : "opacity-100"
        }`}
      />
      <Image
        src={src}
        alt={alt}
        fill
        sizes={sizes}
        className={`object-cover transition-[opacity,scale] duration-500 ease-projection group-hover:scale-[1.03] motion-reduce:transition-none ${
          isLoaded ? (dimmed ? "opacity-40" : "opacity-100") : "opacity-0"
        }`}
        onLoad={() => setIsLoaded(true)}
        onError={() => setHasError(true)}
      />
    </>
  );
}

const badgeTones = {
  neutral: "bg-black/65 text-white",
  muted: "bg-muted/90 text-muted-foreground",
  success: "bg-success/90 text-success-foreground",
  warning: "bg-warning/95 text-warning-foreground",
  orange: "bg-orange-500/95 text-white",
  info: "bg-info/90 text-info-foreground",
  error: "bg-error/90 text-error-foreground",
} as const;

export type CoverBadgeTone = keyof typeof badgeTones;

/** Small pill laid over the cover (format, status, missing volumes…). */
export function CoverBadge({
  tone = "neutral",
  className,
  children,
}: {
  tone?: CoverBadgeTone;
  className?: string;
  children: ReactNode;
}) {
  return (
    <span
      className={`inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-3xs font-bold tracking-wide ${badgeTones[tone]} ${className ?? ""}`}
    >
      {children}
    </span>
  );
}

/**
 * Bottom scrim revealed on hover/focus, or always visible on touch screens
 * (where hover never fires) so the title never becomes unreachable.
 */
export function CoverScrim({
  children,
  className,
  reserveActions = false,
}: {
  children: ReactNode;
  className?: string;
  /** Keep room at the bottom for the always-visible shortcut cluster. */
  reserveActions?: boolean;
}) {
  return (
    <div
      className={`pointer-events-none absolute inset-x-0 bottom-0 translate-y-full bg-gradient-to-t from-black via-black/80 to-transparent px-3 ${
        // The always-visible shortcut cluster sits at `bottom-2` and is `h-7`, so
        // the text block has to end above 36px or the last line slides under the
        // buttons once the scrim is revealed on hover.
        reserveActions ? "pb-11" : "pb-3"
      } pt-10 transition-transform duration-300 ease-out group-focus-within:translate-y-0 group-hover:translate-y-0 max-sm:translate-y-0 ${className ?? ""}`}
    >
      {children}
    </div>
  );
}

/** Thin in-progress bar, as shown under a partially read cover in the reader. */
export function CoverProgress({
  value,
  max,
  label,
}: {
  value: number;
  max: number;
  label: string;
}) {
  if (max <= 0) return null;
  const percent = Math.min(100, Math.max(0, Math.round((value / max) * 100)));
  if (percent <= 0 || percent >= 100) return null;

  return (
    <div
      className="pointer-events-none absolute inset-x-0 bottom-0 h-1.5 bg-black/55"
      role="progressbar"
      aria-valuenow={percent}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={label}
    >
      <div
        className="h-full origin-left bg-primary transition-transform duration-500 ease-out"
        style={{ transform: `scaleX(${percent / 100})` }}
      />
    </div>
  );
}

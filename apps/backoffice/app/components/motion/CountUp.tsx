"use client";

import { animate, useReducedMotion } from "framer-motion";
import { useCallback, useEffect, useRef, useState } from "react";

import { formatBytes, formatNumber } from "@/lib/format";
import { EASE_PROJECTION } from "@/lib/motion";

interface CountUpProps {
  /** Final value the counter lands on. */
  value: number;
  /**
   * How to render the running number. Only serialisable formatting (a `locale`
   * plus a format kind) crosses the Server/Client boundary — a function prop
   * would not survive it.
   */
  format?: "number" | "bytes";
  locale?: string;
  /** Count duration in seconds. */
  duration?: number;
  className?: string;
}

/**
 * A number that rolls up to its value the first time it enters the viewport.
 *
 * The final value is rendered by the server, so the figure is always correct
 * without JavaScript; the count-up is a pure enhancement layered on top when
 * the client mounts (and when the element is actually visible). Reduced motion
 * and environments without `IntersectionObserver` (jsdom) keep the final value
 * and skip the animation entirely.
 */
export function CountUp({
  value,
  format = "number",
  locale,
  duration = 0.9,
  className,
}: CountUpProps) {
  const reduce = useReducedMotion();
  const ref = useRef<HTMLSpanElement>(null);

  const render = useCallback(
    (current: number) => {
      const rounded = Math.round(current);
      return format === "bytes" ? formatBytes(rounded, locale) : formatNumber(rounded, locale ?? "en");
    },
    [format, locale],
  );

  const [display, setDisplay] = useState(() => render(value));

  // Keep the rendered figure in sync when the value (or the locale) changes —
  // e.g. a refresh streams a new figure in — without replaying the count-up.
  // Adjusting state during render is React's documented pattern for this; an
  // effect would cause a cascading render.
  const syncKey = `${locale ?? ""}|${format}|${value}`;
  const [syncedKey, setSyncedKey] = useState(syncKey);
  if (syncedKey !== syncKey) {
    setSyncedKey(syncKey);
    setDisplay(render(value));
  }

  useEffect(() => {
    if (reduce) return;
    if (typeof IntersectionObserver === "undefined") return;

    const element = ref.current;
    if (!element) return;

    let controls: ReturnType<typeof animate> | undefined;

    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        observer.disconnect();
        if (value === 0) return;

        controls = animate(0, value, {
          duration,
          ease: EASE_PROJECTION,
          onUpdate: (current) => setDisplay(render(current)),
          onComplete: () => setDisplay(render(value)),
        });
      },
      { threshold: 0.25 },
    );

    observer.observe(element);

    return () => {
      observer.disconnect();
      controls?.stop();
    };
  }, [duration, reduce, render, value]);

  return (
    <span ref={ref} className={className}>
      {display}
    </span>
  );
}

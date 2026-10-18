"use client";

import { useEffect, useRef } from "react";

interface GlowLayerProps {
  className?: string;
  /** Radius of the highlight, in pixels. */
  radius?: number;
  /** Tint of the highlight. Defaults to the app's primary colour. */
  color?: string;
}

/**
 * A soft highlight that follows the pointer across its host.
 *
 * Self-contained: it listens to `pointermove` on its *parent* element and
 * writes the pointer position into CSS variables on itself, so a host only has
 * to be `group relative` (ideally `overflow-hidden`) and drop `<GlowLayer />`
 * inside — no ref plumbing, no wrapper markup. Revealed on `group-hover` and
 * silent under reduced motion.
 */
export function GlowLayer({
  className,
  radius = 220,
  color = "color-mix(in oklab, var(--color-primary) 16%, transparent)",
}: GlowLayerProps) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const element = ref.current;
    const host = element?.parentElement;
    if (!element || !host) return;
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;

    let frame = 0;
    let nextX = 0;
    let nextY = 0;

    const paint = () => {
      frame = 0;
      element.style.setProperty("--glow-x", `${nextX}px`);
      element.style.setProperty("--glow-y", `${nextY}px`);
    };

    const handleMove = (event: PointerEvent) => {
      const rect = host.getBoundingClientRect();
      nextX = event.clientX - rect.left;
      nextY = event.clientY - rect.top;
      if (!frame) frame = window.requestAnimationFrame(paint);
    };

    host.addEventListener("pointermove", handleMove);
    return () => {
      host.removeEventListener("pointermove", handleMove);
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, []);

  return (
    <span
      ref={ref}
      aria-hidden
      className={`pointer-events-none absolute inset-0 rounded-[inherit] opacity-0 transition-opacity duration-500 group-hover:opacity-100 motion-reduce:hidden ${className ?? ""}`}
      style={{
        background: `radial-gradient(${radius}px circle at var(--glow-x, 50%) var(--glow-y, 50%), ${color}, transparent 65%)`,
      }}
    />
  );
}

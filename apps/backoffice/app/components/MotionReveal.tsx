"use client";

import { useEffect } from "react";
import { usePathname } from "next/navigation";

const REVEALED = "is-revealed";

/**
 * Global scroll-reveal driver.
 *
 * Watches every `[data-reveal]` element and adds `is-revealed` once it enters
 * the viewport (see the `.js-motion [data-reveal]` rules in `globals.css`).
 * Mounted once from the app shell so pages can opt in with a plain attribute
 * and stay Server Components.
 *
 * - No `IntersectionObserver` (e.g. jsdom) → everything is revealed at once.
 * - A debounced `MutationObserver` picks up nodes added later (streamed
 *   Suspense boundaries, client-side pagination).
 */
export function MotionReveal() {
  const pathname = usePathname();

  useEffect(() => {
    const root = document.documentElement;
    // Fallback in case the pre-paint inline script didn't run.
    root.classList.add("js-motion");

    const reveal = (el: Element) => el.classList.add(REVEALED);

    const observer =
      typeof IntersectionObserver === "undefined"
        ? null
        : new IntersectionObserver(
            (entries) => {
              for (const entry of entries) {
                if (!entry.isIntersecting) continue;
                reveal(entry.target);
                observer?.unobserve(entry.target);
              }
            },
            // A large top margin: content that sits above the viewport is
            // already "seen". Without it, a browser restoring the scroll
            // position on back-navigation would leave everything above the
            // fold invisible until the user scrolls up.
            { rootMargin: "4000px 0px -8% 0px", threshold: 0.01 }
          );

    const tracked = new Set<Element>();

    const scan = () => {
      document
        .querySelectorAll("[data-reveal]:not(.is-revealed)")
        .forEach((el) => {
          if (tracked.has(el)) return;
          if (observer) {
            tracked.add(el);
            observer.observe(el);
          } else {
            reveal(el);
          }
        });
    };

    scan();

    let frame = 0;
    const mutations =
      typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver(() => {
            if (frame) return;
            frame = window.requestAnimationFrame(() => {
              frame = 0;
              scan();
            });
          });
    mutations?.observe(document.body, { childList: true, subtree: true });

    return () => {
      observer?.disconnect();
      mutations?.disconnect();
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, [pathname]);

  return null;
}

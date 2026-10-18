"use client";

import { motion, useReducedMotion, useScroll, useSpring } from "framer-motion";

interface ScrollProgressProps {
  className?: string;
}

/**
 * A hairline that fills up with the page's scroll progress.
 *
 * Meant to sit at the bottom edge of the sticky header, so it is always in
 * sight without any `position: fixed` of its own. Purely decorative: hidden
 * under reduced motion.
 */
export function ScrollProgress({ className }: ScrollProgressProps) {
  const reduce = useReducedMotion();
  const { scrollYProgress } = useScroll();
  const scaleX = useSpring(scrollYProgress, { stiffness: 220, damping: 40, mass: 0.3 });

  if (reduce) return null;

  return (
    <motion.div
      aria-hidden
      className={className}
      style={{ scaleX, transformOrigin: "0% 50%" }}
    />
  );
}

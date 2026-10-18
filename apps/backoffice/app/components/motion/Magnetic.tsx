"use client";

import { motion, useReducedMotion, useSpring } from "framer-motion";
import { useRef, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";

interface MagneticProps {
  children: ReactNode;
  className?: string;
  /** Fraction of the pointer offset that is followed (0–1). */
  strength?: number;
  /** Hard cap in pixels so the gesture never looks like a layout bug. */
  max?: number;
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), max);

/**
 * A surface that leans a few pixels toward the pointer, then springs back.
 *
 * The wrapper carries the motion, so the child keeps its own hover/active
 * transforms (independent elements compose instead of overwriting each other).
 * Motionless under reduced motion.
 */
export function Magnetic({ children, className, strength = 0.28, max = 12 }: MagneticProps) {
  const reduce = useReducedMotion();
  const x = useSpring(0, { stiffness: 260, damping: 20, mass: 0.6 });
  const y = useSpring(0, { stiffness: 260, damping: 20, mass: 0.6 });
  const ref = useRef<HTMLDivElement>(null);

  const handleMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (reduce) return;
    const element = ref.current;
    if (!element) return;

    const rect = element.getBoundingClientRect();
    const dx = (event.clientX - (rect.left + rect.width / 2)) * strength;
    const dy = (event.clientY - (rect.top + rect.height / 2)) * strength;
    x.set(clamp(dx, -max, max));
    y.set(clamp(dy, -max, max));
  };

  const reset = () => {
    x.set(0);
    y.set(0);
  };

  return (
    <motion.div
      ref={ref}
      className={className}
      style={{ x, y }}
      onPointerMove={handleMove}
      onPointerLeave={reset}
      onPointerCancel={reset}
    >
      {children}
    </motion.div>
  );
}

"use client";

import type { InputHTMLAttributes, ReactNode } from "react";

type SwitchSize = "sm" | "md";

interface SwitchProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "size"> {
  /** Track/thumb size. `md` (default) for standalone toggles, `sm` for dense lists. */
  size?: SwitchSize;
  /** Optional text rendered beside the switch. When set, the whole row toggles. */
  label?: ReactNode;
  /** Where the text sits relative to the switch. Defaults to `right`. */
  labelPosition?: "left" | "right";
  /** Extra classes for the outer `<label>`. */
  className?: string;
}

const SIZES: Record<SwitchSize, { track: string; thumb: string }> = {
  sm: { track: "w-9 h-5", thumb: "after:h-4 after:w-4" },
  md: { track: "w-11 h-6", thumb: "after:h-5 after:w-5" },
};

export function Switch({ size = "md", label, labelPosition = "right", className = "", ...props }: SwitchProps) {
  const s = SIZES[size];

  const control = (
    <span className="relative inline-flex items-center shrink-0">
      <input type="checkbox" className="sr-only peer" {...props} />
      <span
        className={`${s.track} bg-muted rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:transition-all peer-checked:bg-primary ${s.thumb}`}
      />
    </span>
  );

  const layout =
    label == null
      ? `inline-flex ${className}`
      : `flex items-center gap-3 ${labelPosition === "left" ? "justify-between" : ""} ${className}`;

  return (
    <label className={`cursor-pointer ${layout}`}>
      {labelPosition === "left" ? (
        <>
          {label}
          {control}
        </>
      ) : (
        <>
          {control}
          {label}
        </>
      )}
    </label>
  );
}

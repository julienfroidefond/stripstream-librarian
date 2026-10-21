import { ReactNode } from "react";

interface TooltipProps {
  label: string;
  /** Optional second line for a longer, wrapped explanation. */
  description?: string;
  /** Bubble alignment relative to the trigger. */
  align?: "center" | "start" | "end";
  children: ReactNode;
  className?: string;
}

export function Tooltip({
  label,
  description,
  align = "center",
  children,
  className = "",
}: TooltipProps) {
  const alignment =
    align === "start" ? "left-0" : align === "end" ? "right-0" : "left-1/2 -translate-x-1/2";
  return (
    <span className={`relative group/tooltip inline-flex ${className}`}>
      {children}
      <span
        className={`pointer-events-none absolute bottom-full z-50 mb-2 rounded-lg border border-border bg-popover px-2.5 text-xs text-popover-foreground shadow-lg opacity-0 scale-95 transition-all duration-150 group-hover/tooltip:scale-100 group-hover/tooltip:opacity-100 group-focus-visible/tooltip:scale-100 group-focus-visible/tooltip:opacity-100 ${alignment} ${
          description ? "w-64 whitespace-normal py-1.5" : "whitespace-nowrap py-1"
        }`}
      >
        {description ? (
          <>
            <span className="block font-medium">{label}</span>
            <span className="mt-0.5 block leading-snug text-muted-foreground">{description}</span>
          </>
        ) : (
          label
        )}
      </span>
    </span>
  );
}

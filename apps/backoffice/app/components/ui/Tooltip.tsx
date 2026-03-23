import { ReactNode } from "react";

interface TooltipProps {
  label: string;
  children: ReactNode;
  className?: string;
}

export function Tooltip({ label, children, className = "" }: TooltipProps) {
  return (
    <span className={`relative group/tooltip inline-flex ${className}`}>
      {children}
      <span className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2.5 py-1 text-xs text-popover-foreground bg-popover border border-border rounded-lg shadow-lg whitespace-nowrap opacity-0 scale-95 transition-all duration-150 group-hover/tooltip:opacity-100 group-hover/tooltip:scale-100 z-50">
        {label}
      </span>
    </span>
  );
}

import { ReactNode } from "react";

interface StatBoxProps {
  value: ReactNode;
  label: string;
  variant?: "default" | "primary" | "success" | "warning" | "error";
  icon?: ReactNode;
  className?: string;
}

const variantStyles: Record<string, string> = {
  default: "bg-muted/50",
  primary: "bg-primary/10",
  success: "bg-success/10",
  warning: "bg-warning/10",
  error: "bg-destructive/10",
};

const valueVariantStyles: Record<string, string> = {
  default: "text-foreground",
  primary: "text-primary",
  success: "text-success",
  warning: "text-warning",
  error: "text-destructive",
};

export function StatBox({ value, label, variant = "default", icon, className = "" }: StatBoxProps) {
  return (
    <div className={`text-center p-4 rounded-lg transition-colors duration-200 ${variantStyles[variant]} ${className}`}>
      <div className={`flex items-center justify-center gap-1.5 ${valueVariantStyles[variant]}`}>
        {icon && <span className="text-xl">{icon}</span>}
        {/* The figure rises from under its own line — a masked, one-shot entry. */}
        <span className="block overflow-hidden pb-0.5">
          <span className="block text-3xl font-bold animate-count-in">{value}</span>
        </span>
      </div>
      <span className={`text-xs text-muted-foreground`}>{label}</span>
    </div>
  );
}

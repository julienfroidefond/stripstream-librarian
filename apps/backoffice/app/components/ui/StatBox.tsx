import { ReactNode } from "react";

interface StatBoxProps {
  value: ReactNode;
  label: string;
  variant?: "default" | "primary" | "success" | "warning" | "error";
  className?: string;
}

const variantStyles: Record<string, string> = {
  default: "bg-muted/5",
  primary: "bg-primary-soft",
  success: "bg-success-soft",
  warning: "bg-warning-soft",
  error: "bg-error-soft",
};

const valueVariantStyles: Record<string, string> = {
  default: "text-foreground",
  primary: "text-primary",
  success: "text-success",
  warning: "text-warning",
  error: "text-error",
};

export function StatBox({ value, label, variant = "default", className = "" }: StatBoxProps) {
  return (
    <div className={`text-center p-4 rounded-lg ${variantStyles[variant]} ${className}`}>
      <span className={`block text-3xl font-bold ${valueVariantStyles[variant]}`}>{value}</span>
      <span className={`text-xs ${valueVariantStyles[variant]}/80`}>{label}</span>
    </div>
  );
}

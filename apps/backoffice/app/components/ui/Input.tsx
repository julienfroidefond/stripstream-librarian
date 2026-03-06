import { InputHTMLAttributes, SelectHTMLAttributes, ReactNode } from "react";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
}

export function Input({ label, className = "", ...props }: InputProps) {
  return (
    <input
      className={`px-4 py-2.5 rounded-lg border border-line bg-background text-foreground placeholder-muted focus:ring-2 focus:ring-primary focus:border-primary ${className}`}
      {...props}
    />
  );
}

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  children: ReactNode;
}

export function Select({ label, children, className = "", ...props }: SelectProps) {
  return (
    <select
      className={`px-4 py-2.5 rounded-lg border border-line bg-background text-foreground focus:ring-2 focus:ring-primary focus:border-primary ${className}`}
      {...props}
    >
      {children}
    </select>
  );
}

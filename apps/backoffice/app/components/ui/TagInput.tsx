"use client";

import { type KeyboardEvent, type ReactNode } from "react";

const pillClasses = {
  primary: "bg-primary/10 text-primary",
  secondary: "bg-secondary/50 text-secondary-foreground",
  success: "bg-success/10 text-success",
} as const;

const inputClass = "flex h-10 rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50";

interface TagInputProps {
  values: string[];
  input: string;
  onInputChange: (value: string) => void;
  onAdd: () => void;
  onRemove: (index: number) => void;
  removeLabel: (value: string) => string;
  placeholder: string;
  disabled?: boolean;
  variant?: keyof typeof pillClasses;
  inputRef?: (el: HTMLInputElement | null) => void;
  onKeyDown?: (e: KeyboardEvent<HTMLInputElement>) => void;
  onFocus?: () => void;
  autoComplete?: string;
  suggestions?: ReactNode;
  extra?: ReactNode;
}

export function TagInput({
  values,
  input,
  onInputChange,
  onAdd,
  onRemove,
  removeLabel,
  placeholder,
  disabled,
  variant = "primary",
  inputRef,
  onKeyDown,
  onFocus,
  autoComplete,
  suggestions,
  extra,
}: TagInputProps) {
  const field = (
    <input
      ref={inputRef}
      value={input}
      onChange={(e) => onInputChange(e.target.value)}
      onKeyDown={onKeyDown}
      onFocus={onFocus}
      disabled={disabled}
      placeholder={placeholder}
      autoComplete={autoComplete}
      className={`${inputClass} ${suggestions ? "w-full" : "flex-1"}`}
    />
  );

  return (
    <div className="space-y-2">
      {values.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {values.map((value, i) => (
            <span
              key={i}
              className={`inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-medium ${pillClasses[variant]}`}
            >
              {value}
              <button
                type="button"
                onClick={() => onRemove(i)}
                disabled={disabled}
                className="hover:text-destructive transition-colors ml-0.5"
                aria-label={removeLabel(value)}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="flex gap-2">
        {suggestions ? <div className="relative flex-1">{field}{suggestions}</div> : field}
        <button
          type="button"
          onClick={onAdd}
          disabled={disabled || !input.trim()}
          className="px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
        >
          +
        </button>
        {extra}
      </div>
    </div>
  );
}

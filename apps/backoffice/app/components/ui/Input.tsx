import { InputHTMLAttributes, SelectHTMLAttributes, TextareaHTMLAttributes, ReactNode, forwardRef } from "react";

// Input Component
export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ label, error, className = "", ...props }, ref) => {
    return (
      <div className="w-full">
        {label && (
          <label className="block text-sm font-medium text-foreground mb-1.5">
            {label}
          </label>
        )}
        <input
          ref={ref}
          className={`
            flex w-full 
            h-10 px-3 py-2
            rounded-md border border-input 
            bg-background 
            text-sm text-foreground
            shadow-sm
            transition-colors duration-200
            file:border-0 file:bg-transparent file:text-sm file:font-medium 
            placeholder:text-muted-foreground/90
            focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
            disabled:cursor-not-allowed disabled:opacity-50
            ${error ? "border-destructive focus-visible:ring-destructive" : ""}
            ${className}
          `}
          {...props}
        />
        {error && (
          <p className="text-xs text-destructive mt-1">{error}</p>
        )}
      </div>
    );
  }
);
Input.displayName = "Input";

// Select Component
export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
  children: ReactNode;
}

export const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ label, error, children, className = "", ...props }, ref) => {
    return (
      <div className="w-full">
        {label && (
          <label className="block text-sm font-medium text-foreground mb-1.5">
            {label}
          </label>
        )}
        <select
          ref={ref}
          className={`
            flex w-full 
            h-10 px-3 py-2
            rounded-md border border-input 
            bg-background 
            text-sm text-foreground
            shadow-sm
            transition-colors duration-200
            focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
            disabled:cursor-not-allowed disabled:opacity-50
            ${error ? "border-destructive focus-visible:ring-destructive" : ""}
            ${className}
          `}
          {...props}
        >
          {children}
        </select>
        {error && (
          <p className="text-xs text-destructive mt-1">{error}</p>
        )}
      </div>
    );
  }
);
Select.displayName = "Select";

// Textarea Component
export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ label, error, className = "", ...props }, ref) => {
    return (
      <div className="w-full">
        {label && (
          <label className="block text-sm font-medium text-foreground mb-1.5">
            {label}
          </label>
        )}
        <textarea
          ref={ref}
          className={`
            flex w-full 
            min-h-[80px] px-3 py-2
            rounded-md border border-input 
            bg-background 
            text-sm text-foreground
            shadow-sm
            transition-colors duration-200
            placeholder:text-muted-foreground/90
            focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
            disabled:cursor-not-allowed disabled:opacity-50
            resize-vertical
            ${error ? "border-destructive focus-visible:ring-destructive" : ""}
            ${className}
          `}
          {...props}
        />
        {error && (
          <p className="text-xs text-destructive mt-1">{error}</p>
        )}
      </div>
    );
  }
);
Textarea.displayName = "Textarea";

// Search Input with Icon
interface SearchInputProps extends InputHTMLAttributes<HTMLInputElement> {
  icon?: ReactNode;
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(
  ({ icon, className = "", ...props }, ref) => {
    return (
      <div className="relative">
        {icon && (
          <div className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
            {icon}
          </div>
        )}
        <input
          ref={ref}
          className={`
            flex w-full 
            h-10 pl-10 pr-4 py-2
            rounded-md border border-input 
            bg-background 
            text-sm text-foreground
            shadow-sm
            transition-colors duration-200
            placeholder:text-muted-foreground/90
            focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
            disabled:cursor-not-allowed disabled:opacity-50
            ${className}
          `}
          {...props}
        />
      </div>
    );
  }
);
SearchInput.displayName = "SearchInput";

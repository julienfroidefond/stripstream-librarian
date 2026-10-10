import { ReactNode, LabelHTMLAttributes, InputHTMLAttributes, SelectHTMLAttributes } from "react";

// Form Field Container
interface FormFieldProps {
  children: ReactNode;
  className?: string;
}

export function FormField({ children, className = "" }: FormFieldProps) {
  return <div className={`flex flex-col space-y-1.5 ${className}`}>{children}</div>;
}

// Form Label
type FormLabelVariant = "default" | "settings";

interface FormLabelProps extends LabelHTMLAttributes<HTMLLabelElement> {
  children: ReactNode;
  required?: boolean;
  /** `settings` matches the settings form style (muted, block). */
  variant?: FormLabelVariant;
}

const FORM_LABEL_VARIANTS: Record<FormLabelVariant, string> = {
  default: "text-sm font-medium text-foreground leading-none",
  settings: "text-sm font-medium text-muted-foreground mb-1 block",
};

export function FormLabel({ children, required, variant = "default", className = "", ...props }: FormLabelProps) {
  return (
    <label 
      className={`${FORM_LABEL_VARIANTS[variant]} peer-disabled:cursor-not-allowed peer-disabled:opacity-70 ${className}`} 
      {...props}
    >
      {children}
      {required && <span className="text-destructive ml-1">*</span>}
    </label>
  );
}

// Form Input
interface FormInputProps extends InputHTMLAttributes<HTMLInputElement> {
  error?: string;
}

export function FormInput({ className = "", error, ...props }: FormInputProps) {
  return (
    <input
      className={`
        flex h-10 w-full 
        rounded-md border border-input 
        bg-background px-3 py-2 
        text-sm 
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
  );
}

// Form Select
interface FormSelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  children: ReactNode;
  error?: string;
}

export function FormSelect({ children, className = "", error, ...props }: FormSelectProps) {
  return (
    <select
      className={`
        flex h-10 w-full 
        rounded-md border border-input 
        bg-background px-3 py-2 
        text-sm 
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
  );
}

// Form Row (horizontal layout)
interface FormRowProps {
  children: ReactNode;
  className?: string;
}

export function FormRow({ children, className = "" }: FormRowProps) {
  return <div className={`flex flex-wrap items-start gap-4 ${className}`}>{children}</div>;
}

// Form Section
interface FormSectionProps {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}

export function FormSection({ title, description, children, className = "" }: FormSectionProps) {
  return (
    <div className={`space-y-4 ${className}`}>
      {(title || description) && (
        <div className="space-y-1">
          {title && <h3 className="text-lg font-medium text-foreground">{title}</h3>}
          {description && <p className="text-sm text-muted-foreground">{description}</p>}
        </div>
      )}
      <div className="space-y-4">
        {children}
      </div>
    </div>
  );
}

// Form Error Message
interface FormErrorProps {
  children: ReactNode;
  className?: string;
}

export function FormError({ children, className = "" }: FormErrorProps) {
  return (
    <p className={`text-xs text-destructive ${className}`}>
      {children}
    </p>
  );
}

// Form Description
interface FormDescriptionProps {
  children: ReactNode;
  className?: string;
}

export function FormDescription({ children, className = "" }: FormDescriptionProps) {
  return (
    <p className={`text-xs text-muted-foreground ${className}`}>
      {children}
    </p>
  );
}

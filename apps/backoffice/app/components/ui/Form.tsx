import { ReactNode, LabelHTMLAttributes, InputHTMLAttributes, SelectHTMLAttributes } from "react";

interface FormFieldProps {
  children: ReactNode;
  className?: string;
}

export function FormField({ children, className = "" }: FormFieldProps) {
  return <div className={`flex-1 min-w-48 ${className}`}>{children}</div>;
}

interface FormLabelProps extends LabelHTMLAttributes<HTMLLabelElement> {
  children: ReactNode;
}

export function FormLabel({ children, className = "", ...props }: FormLabelProps) {
  return (
    <label className={`block text-sm font-medium text-foreground mb-1.5 ${className}`} {...props}>
      {children}
    </label>
  );
}

interface FormInputProps extends InputHTMLAttributes<HTMLInputElement> {}

export function FormInput({ className = "", ...props }: FormInputProps) {
  return (
    <input
      className={`w-full h-10 px-3 rounded-lg border border-line bg-background text-foreground placeholder-muted focus:ring-2 focus:ring-primary focus:border-primary transition-colors text-sm ${className}`}
      {...props}
    />
  );
}

interface FormSelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  children: ReactNode;
}

export function FormSelect({ children, className = "", ...props }: FormSelectProps) {
  return (
    <select
      className={`w-full h-10 px-3 rounded-lg border border-line bg-background text-foreground focus:ring-2 focus:ring-primary focus:border-primary transition-colors text-sm ${className}`}
      {...props}
    >
      {children}
    </select>
  );
}

interface FormRowProps {
  children: ReactNode;
  className?: string;
}

export function FormRow({ children, className = "" }: FormRowProps) {
  return <div className={`flex items-end gap-3 flex-wrap ${className}`}>{children}</div>;
}

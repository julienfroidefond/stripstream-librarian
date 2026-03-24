import { ButtonHTMLAttributes, ReactNode } from "react";

type ButtonVariant = 
  | "default" 
  | "destructive" 
  | "outline" 
  | "secondary" 
  | "ghost" 
  | "link"
  | "primary"
  | "danger"
  | "warning";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  variant?: ButtonVariant;
  size?: "xs" | "sm" | "md" | "lg";
}

const variantStyles: Record<ButtonVariant, string> = {
  // shadcn/ui compatible variants
  default: "bg-primary text-primary-foreground hover:bg-primary/90 shadow-sm hover:shadow-md",
  destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90 shadow-sm",
  outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
  secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/85 shadow-sm",
  ghost: "hover:bg-accent hover:text-accent-foreground",
  link: "text-primary underline-offset-4 hover:underline",
  
  // Legacy variants (mapped to new ones for compatibility)
  primary: "bg-primary text-primary-foreground hover:bg-primary/90 shadow-sm hover:shadow-md",
  danger: "bg-destructive text-destructive-foreground hover:bg-destructive/90 shadow-sm",
  warning: "bg-warning text-white hover:bg-warning/90 shadow-sm",
};

const sizeStyles: Record<string, string> = {
  xs: "h-7 px-2.5 text-xs rounded-md",
  sm: "h-9 px-3 text-xs rounded-md",
  md: "h-10 px-4 py-2 text-sm rounded-md",
  lg: "h-11 px-8 text-base rounded-md",
};

export function Button({ 
  children, 
  variant = "default", 
  size = "md", 
  className = "",
  disabled,
  ...props 
}: ButtonProps) {
  return (
    <button 
      className={`
        inline-flex items-center justify-center
        font-medium cursor-pointer
        transition-all duration-200 ease-out
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
        disabled:pointer-events-none disabled:opacity-50
        active:scale-[0.98]
        ${variantStyles[variant]}
        ${sizeStyles[size]}
        ${className}
      `}
      disabled={disabled}
      {...props}
    >
      {children}
    </button>
  );
}

// Icon Button variant
interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  size?: "sm" | "md" | "lg";
  variant?: ButtonVariant;
  title?: string;
}

const iconSizeStyles: Record<string, string> = {
  sm: "h-8 w-8",
  md: "h-9 w-9",
  lg: "h-10 w-10",
};

export function IconButton({ 
  children, 
  size = "md", 
  variant = "ghost",
  className = "",
  title,
  ...props 
}: IconButtonProps) {
  return (
    <button 
      title={title}
      className={`
        inline-flex items-center justify-center 
        rounded-md
        transition-all duration-200 ease-out
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
        disabled:pointer-events-none disabled:opacity-50
        hover:bg-accent hover:text-accent-foreground
        active:scale-[0.96]
        ${iconSizeStyles[size]}
        ${className}
      `}
      {...props}
    >
      {children}
    </button>
  );
}

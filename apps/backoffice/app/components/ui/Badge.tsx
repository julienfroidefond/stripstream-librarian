import { ReactNode } from "react";

type BadgeVariant = 
  | "default" 
  | "primary" 
  | "secondary"
  | "destructive"
  | "outline"
  | "success" 
  | "warning" 
  | "error" 
  | "muted"
  | "unread"
  | "in-progress"
  | "completed";

interface BadgeProps {
  children: ReactNode;
  variant?: BadgeVariant;
  className?: string;
}

const variantStyles: Record<BadgeVariant, string> = {
  // shadcn/ui compatible
  default: "bg-primary/90 text-primary-foreground border-transparent hover:bg-primary/80 backdrop-blur-md",
  secondary: "bg-secondary/80 text-secondary-foreground border-transparent hover:bg-secondary/60 backdrop-blur-md",
  destructive: "bg-destructive/90 text-destructive-foreground border-transparent hover:bg-destructive/80 backdrop-blur-md",
  outline: "text-foreground border-border bg-background/50",
  
  // Legacy + Additional variants
  primary: "bg-primary/90 text-primary-foreground backdrop-blur-md",
  success: "bg-success/90 text-success-foreground backdrop-blur-md",
  warning: "bg-warning/90 text-white backdrop-blur-md",
  error: "bg-destructive/90 text-destructive-foreground backdrop-blur-md",
  muted: "bg-muted/60 text-muted-foreground backdrop-blur-md",
  
  // Status badges from StripStream
  unread: "badge-unread backdrop-blur-md",
  "in-progress": "badge-in-progress backdrop-blur-md",
  completed: "badge-completed backdrop-blur-md",
};

export function Badge({ children, variant = "default", className = "" }: BadgeProps) {
  return (
    <span className={`
      inline-flex items-center 
      px-2.5 py-0.5 
      rounded-full 
      text-xs font-semibold 
      border
      transition-colors duration-200
      ${variantStyles[variant]} 
      ${className}
    `}>
      {children}
    </span>
  );
}

// Status badge for jobs/tasks
const statusVariants: Record<string, BadgeVariant> = {
  running: "in-progress",
  success: "completed",
  completed: "completed",
  failed: "error",
  cancelled: "muted",
  pending: "warning",
  unread: "unread",
};

interface StatusBadgeProps {
  status: string;
  className?: string;
}

export function StatusBadge({ status, className = "" }: StatusBadgeProps) {
  const variant = statusVariants[status.toLowerCase()] || "default";
  return <Badge variant={variant} className={className}>{status}</Badge>;
}

// Job type badge
const jobTypeVariants: Record<string, BadgeVariant> = {
  rebuild: "primary",
  full_rebuild: "warning",
};

interface JobTypeBadgeProps {
  type: string;
  className?: string;
}

export function JobTypeBadge({ type, className = "" }: JobTypeBadgeProps) {
  const variant = jobTypeVariants[type.toLowerCase()] || "default";
  return <Badge variant={variant} className={className}>{type}</Badge>;
}

// Progress badge (shows percentage)
interface ProgressBadgeProps {
  progress: number;
  className?: string;
}

export function ProgressBadge({ progress, className = "" }: ProgressBadgeProps) {
  let variant: BadgeVariant = "unread";
  if (progress === 100) variant = "completed";
  else if (progress > 0) variant = "in-progress";
  
  return (
    <Badge variant={variant} className={className}>
      {progress}%
    </Badge>
  );
}

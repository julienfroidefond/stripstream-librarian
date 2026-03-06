type BadgeVariant = "default" | "primary" | "success" | "warning" | "error" | "muted";

interface BadgeProps {
  children: React.ReactNode;
  variant?: BadgeVariant;
  className?: string;
}

const variantStyles: Record<BadgeVariant, string> = {
  default: "bg-muted/20 text-muted",
  primary: "bg-primary-soft text-primary",
  success: "bg-success-soft text-success",
  warning: "bg-warning-soft text-warning",
  error: "bg-error-soft text-error",
  muted: "bg-muted/10 text-muted",
};

export function Badge({ children, variant = "default", className = "" }: BadgeProps) {
  return (
    <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-semibold ${variantStyles[variant]} ${className}`}>
      {children}
    </span>
  );
}

type StatusVariant = "running" | "success" | "failed" | "cancelled" | "pending";

interface StatusBadgeProps {
  status: string;
  className?: string;
}

const statusVariants: Record<StatusVariant, BadgeVariant> = {
  running: "primary",
  success: "success",
  failed: "error",
  cancelled: "muted",
  pending: "warning",
};

export function StatusBadge({ status, className = "" }: StatusBadgeProps) {
  const variant = statusVariants[status as StatusVariant] || "default";
  return <Badge variant={variant} className={className}>{status}</Badge>;
}

type JobTypeVariant = "rebuild" | "full_rebuild";

interface JobTypeBadgeProps {
  type: string;
  className?: string;
}

const jobTypeVariants: Record<JobTypeVariant, BadgeVariant> = {
  rebuild: "primary",
  full_rebuild: "warning",
};

export function JobTypeBadge({ type, className = "" }: JobTypeBadgeProps) {
  const variant = jobTypeVariants[type as JobTypeVariant] || "default";
  return <Badge variant={variant} className={className}>{type}</Badge>;
}

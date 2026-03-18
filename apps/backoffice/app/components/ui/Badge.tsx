"use client";

import { ReactNode } from "react";
import { useTranslation } from "../../../lib/i18n/context";

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
  extracting_pages: "in-progress",
  generating_thumbnails: "in-progress",
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
  const { t } = useTranslation();
  const key = status.toLowerCase();
  const variant = statusVariants[key] || "default";
  const statusLabels: Record<string, string> = {
    extracting_pages: t("statusBadge.extracting_pages"),
    generating_thumbnails: t("statusBadge.generating_thumbnails"),
  };
  const label = statusLabels[key] ?? status;
  return <Badge variant={variant} className={className}>{label}</Badge>;
}

// Job type badge
const jobTypeVariants: Record<string, BadgeVariant> = {
  rebuild: "primary",
  full_rebuild: "warning",
  thumbnail_rebuild: "secondary",
  thumbnail_regenerate: "warning",
};

interface JobTypeBadgeProps {
  type: string;
  className?: string;
}

export function JobTypeBadge({ type, className = "" }: JobTypeBadgeProps) {
  const { t } = useTranslation();
  const key = type.toLowerCase();
  const variant = jobTypeVariants[key] || "default";
  const jobTypeLabels: Record<string, string> = {
    rebuild: t("jobType.rebuild"),
    full_rebuild: t("jobType.full_rebuild"),
    thumbnail_rebuild: t("jobType.thumbnail_rebuild"),
    thumbnail_regenerate: t("jobType.thumbnail_regenerate"),
    cbr_to_cbz: t("jobType.cbr_to_cbz"),
    metadata_batch: t("jobType.metadata_batch"),
  };
  const label = jobTypeLabels[key] ?? type;
  return <Badge variant={variant} className={className}>{label}</Badge>;
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

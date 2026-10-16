"use client";

import type { ReactNode } from "react";

import { FormField, FormLabel } from "./Form";
import { Icon, type IconName } from "./Icon";

interface SettingsCardProps {
  /** Leading icon rendered next to the title. */
  icon?: IconName;
  /** Section title. Omit for flush panels such as tables. */
  title?: ReactNode;
  description?: ReactNode;
  /** Trailing element aligned with the title (e.g. a switch or a badge). */
  action?: ReactNode;
  children?: ReactNode;
  /** Classes for the outer panel (defaults to `mb-6`; pass `""` inside a spaced container). */
  className?: string;
  /** Padding around the body. Pass `p-0` for flush content such as tables. */
  bodyClassName?: string;
  /** Spacing between the body children (defaults to `space-y-4`). */
  contentClassName?: string;
}

/**
 * Settings section: a quiet bordered panel with an optional leading icon, a title,
 * a one-line description and a spaced content body. No hover lift — these are
 * static surfaces, not cards you click.
 */
export function SettingsCard({
  icon,
  title,
  description,
  action,
  children,
  className = "mb-6",
  bodyClassName = "px-5 py-5",
  contentClassName = "space-y-4",
}: SettingsCardProps) {
  const hasHeader = title != null || description != null || action != null;
  return (
    <section className={`overflow-hidden rounded-xl border border-border/70 bg-card text-card-foreground shadow-soft ${className}`}>
      {hasHeader && (
        <header className="flex items-start gap-3 border-b border-border/50 bg-muted/40 px-5 py-4">
          {icon && (
            <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-inset ring-primary/20">
              <Icon name={icon} size="sm" mono />
            </span>
          )}
          <div className="min-w-0 flex-1">
            {title != null && <h2 className="text-base font-semibold leading-6 tracking-tight text-foreground">{title}</h2>}
            {description != null && (
              <p className={`text-sm leading-relaxed text-muted-foreground ${title != null ? "mt-1" : ""}`}>{description}</p>
            )}
          </div>
          {action != null && <div className="shrink-0">{action}</div>}
        </header>
      )}
      {children != null && (
        <div className={bodyClassName}>
          <div className={contentClassName}>{children}</div>
        </div>
      )}
    </section>
  );
}

interface SettingsRowProps {
  /** Row title, rendered in a left gutter (stacked above on small screens). */
  label: ReactNode;
  /** Secondary line under the label. */
  description?: ReactNode;
  /** Associates the label with a control id. */
  htmlFor?: string;
  /** The control, rendered in the right column. */
  children: ReactNode;
  className?: string;
  /** Extra classes for the control column. */
  controlClassName?: string;
}

/**
 * A label/control pair laid out as a two-column row (label on the left, control on
 * the right) — the classic settings layout that keeps long forms scannable.
 */
export function SettingsRow({
  label,
  description,
  htmlFor,
  children,
  className = "",
  controlClassName = "",
}: SettingsRowProps) {
  return (
    <div className={`grid gap-2 sm:grid-cols-[minmax(0,13rem)_minmax(0,1fr)] sm:items-start sm:gap-6 ${className}`}>
      <div className="space-y-0.5 sm:pt-2">
        <label htmlFor={htmlFor} className="block text-sm font-medium leading-5 text-foreground">
          {label}
        </label>
        {description != null && (
          <p className="text-xs leading-relaxed text-muted-foreground">{description}</p>
        )}
      </div>
      <div className={`min-w-0 sm:max-w-md ${controlClassName}`}>{children}</div>
    </div>
  );
}

interface SettingsFieldProps {
  label: ReactNode;
  children: ReactNode;
  /** Small helper text rendered under the control. */
  help?: ReactNode;
  /** Classes for the `FormField` wrapper. */
  className?: string;
  /** Extra label classes (icon rows, fixed widths, …). */
  labelClassName?: string;
  /** Associates the label with a control id. */
  labelHtmlFor?: string;
}

/** Label + control pair using the shared settings label style. */
export function SettingsField({ label, children, help, className = "", labelClassName = "", labelHtmlFor }: SettingsFieldProps) {
  return (
    <FormField className={className}>
      <FormLabel variant="settings" className={labelClassName} htmlFor={labelHtmlFor}>
        {label}
      </FormLabel>
      {children}
      {help != null && <p className="text-xs text-muted-foreground mt-1">{help}</p>}
    </FormField>
  );
}

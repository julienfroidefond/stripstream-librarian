"use client";

import { Children, type ReactNode } from "react";

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
  /** Extra classes for the outer panel (spacing is owned by `SettingsSections`). */
  className?: string;
  /** Padding around the body. Pass `p-0` for flush content such as tables. */
  bodyClassName?: string;
  /** Spacing between the body children (defaults to `space-y-4`). */
  contentClassName?: string;
}

/**
 * Settings section: a filled panel (tone instead of borders), an optional leading
 * icon, a title, a one-line description and a spaced content body. No hover lift —
 * these are static surfaces, not cards you click.
 *
 * Spacing between sections is owned by `SettingsSections`: the panel itself carries
 * no margin so it can flow in a multi-column layout.
 */
export function SettingsCard({
  icon,
  title,
  description,
  action,
  children,
  className = "",
  bodyClassName,
  contentClassName = "space-y-4",
}: SettingsCardProps) {
  const hasHeader = title != null || description != null || action != null;
  const bodyPadding = bodyClassName ?? (hasHeader ? "px-5 pb-5 pt-3" : "p-5");
  return (
    <section className={`settings-panel overflow-hidden rounded-2xl text-card-foreground ${className}`}>
      {hasHeader && (
        <header className="flex items-start gap-2.5 px-5 pt-5">
          {icon && <Icon name={icon} size="sm" mono className="mt-0.5 shrink-0 text-primary" />}
          <div className="min-w-0 flex-1">
            {title != null && <h2 className="text-[15px] font-semibold leading-6 tracking-tight text-foreground">{title}</h2>}
            {description != null && (
              <p className={`text-[13px] leading-relaxed text-muted-foreground ${title != null ? "mt-0.5" : ""}`}>{description}</p>
            )}
          </div>
          {action != null && <div className="shrink-0">{action}</div>}
        </header>
      )}
      {children != null && (
        <div className={bodyPadding}>
          <div className={contentClassName}>{children}</div>
        </div>
      )}
    </section>
  );
}

interface SettingsSectionsProps {
  children: ReactNode;
  /**
   * `stack` keeps a single full-width column (tables, long vertical forms). Components
   * that already lay out their own panels side by side (Telegram, AI tagging) also use it.
   */
  layout?: "columns" | "stack";
  className?: string;
}

/**
 * Vertical rhythm for a settings page, and — from `xl` up — a two-column flow so
 * wide screens stop leaving a void on the right. Implemented with CSS multi-column
 * (`break-inside-avoid`) so panels of different heights nest without grid gaps.
 *
 * Each child is wrapped in its own unbreakable spacer: the rhythm comes from a bottom
 * *padding* (never a margin) because WebKit carries a trailing margin across the
 * column break and would otherwise push the second column down by one gap.
 */
export function SettingsSections({ children, layout = "columns", className = "" }: SettingsSectionsProps) {
  return (
    <div className={`columns-1 gap-5 ${layout === "columns" ? "xl:columns-2" : ""} ${className}`}>
      {Children.map(children, (child, i) =>
        child == null ? null : (
          <div key={i} className="break-inside-avoid pb-5 [&>*+*]:mt-5">
            {child}
          </div>
        ),
      )}
    </div>
  );
}

interface SettingsColumnsProps {
  children: ReactNode;
  className?: string;
}

/**
 * Explicit two-column (xl) split for tabs whose panels fall into named groups —
 * e.g. "where releases come from" beside "where they go". Unlike
 * `SettingsSections`, which auto-balances a single flow, each child here is one
 * fixed column and keeps its own top-to-bottom order. Stacks below `xl`.
 */
export function SettingsColumns({ children, className = "" }: SettingsColumnsProps) {
  return <div className={`grid items-start gap-5 xl:grid-cols-2 ${className}`}>{children}</div>;
}

/** One column of `SettingsColumns`: a vertical flow with the section rhythm. */
export function SettingsColumn({ children, className = "" }: SettingsColumnsProps) {
  return <div className={`space-y-5 ${className}`}>{children}</div>;
}

interface SettingsSectionProps {
  /**
   * Eyebrow heading naming the group. Card titles are already `<h2>`, so this is a
   * label above them rather than a second title level.
   */
  title: ReactNode;
  children: ReactNode;
  className?: string;
}

/**
 * A named group of settings panels inside a tab: an eyebrow heading and a hairline
 * rule, then a vertical flow of panels. Use it when a tab holds several kinds of
 * thing — where releases come from, where they go, what fetches them.
 *
 * The tab owns the spacing *between* sections (`space-y-8`); this component owns the
 * spacing inside it.
 */
export function SettingsSection({ title, children, className = "" }: SettingsSectionProps) {
  return (
    <section className={`space-y-3 ${className}`}>
      <div className="flex items-center gap-3">
        <h2 className="text-2xs font-semibold uppercase tracking-wider text-muted-foreground">{title}</h2>
        <span aria-hidden className="h-px flex-1 bg-border/60" />
      </div>
      <div className="space-y-5">{children}</div>
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
    <div className={`grid gap-2 py-1.5 sm:grid-cols-[minmax(0,1fr)_minmax(0,20rem)] sm:items-start sm:gap-8 ${className}`}>
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

"use client";

import type { ReactNode } from "react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./Card";
import { FormField, FormLabel } from "./Form";
import { Icon, type IconName } from "./Icon";

interface SettingsCardProps {
  /** Leading icon rendered next to the title. */
  icon?: IconName;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  /** Classes for the outer `Card` (defaults to `mb-6`). */
  className?: string;
  /** Classes for the content wrapper (defaults to `space-y-4`). */
  contentClassName?: string;
}

/** Settings section: a `Card` with an optional icon, title, description and a spaced content body. */
export function SettingsCard({
  icon,
  title,
  description,
  children,
  className = "mb-6",
  contentClassName = "space-y-4",
}: SettingsCardProps) {
  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {icon && <Icon name={icon} size="md" />}
          {title}
        </CardTitle>
        {description != null && <CardDescription>{description}</CardDescription>}
      </CardHeader>
      <CardContent className={contentClassName}>{children}</CardContent>
    </Card>
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

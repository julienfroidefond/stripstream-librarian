import type { ReactNode } from "react";

import { Icon, type IconName } from "./Icon";

interface SettingsPageHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  /** Leading icon shown next to the title, like the other page headers. */
  icon?: IconName;
  /** Optional trailing element aligned with the title (e.g. a status or action). */
  actions?: ReactNode;
}

/** Page title block for the settings page: title, one-line description, optional actions. */
export function SettingsPageHeader({ title, description, icon, actions }: SettingsPageHeaderProps) {
  return (
    <div className="mb-6 flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
      <div className="min-w-0">
        <h1 className="flex items-center gap-3 text-3xl font-bold tracking-tight text-foreground">
          {icon != null && <Icon name={icon} size="xl" mono className="shrink-0 text-primary" />}
          {title}
        </h1>
        {description != null && (
          <p className="mt-2 max-w-2xl text-sm leading-relaxed text-muted-foreground">{description}</p>
        )}
      </div>
      {actions != null && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

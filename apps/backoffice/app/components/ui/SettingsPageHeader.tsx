import type { ReactNode } from "react";

interface SettingsPageHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  /** Optional trailing element aligned with the title (e.g. a status or action). */
  actions?: ReactNode;
}

/** Page title block for the settings page: title, one-line description, optional actions. */
export function SettingsPageHeader({ title, description, actions }: SettingsPageHeaderProps) {
  return (
    <div className="mb-6 flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
      <div className="min-w-0">
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
        {description != null && (
          <p className="mt-1.5 max-w-2xl text-sm leading-relaxed text-muted-foreground">{description}</p>
        )}
      </div>
      {actions != null && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

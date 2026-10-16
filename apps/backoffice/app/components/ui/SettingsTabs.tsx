"use client";

import type { ReactNode } from "react";

import { Icon, type IconName } from "./Icon";

export interface SettingsTabItem {
  id: string;
  label: string;
  icon?: IconName;
}

interface SettingsTabsProps {
  tabs: readonly SettingsTabItem[];
  active: string;
  onChange: (id: string) => void;
  /** Trailing slot that stays in view while the tab strip scrolls (e.g. a saving indicator). */
  trailing?: ReactNode;
}

/**
 * Horizontal tab bar for the settings page, sticky under the global nav so the
 * section switcher stays reachable on long tabs.
 *
 * Deliberately keeps plain `<button>` elements (no `role="tab"`): the accessible
 * name stays the label, which the e2e suite relies on.
 */
export function SettingsTabs({ tabs, active, onChange, trailing }: SettingsTabsProps) {
  return (
    <nav
      aria-label="Settings sections"
      className="sticky top-12 z-30 -mt-1 mb-6 border-b border-border/60 bg-background/85 backdrop-blur-md"
    >
      <div className="flex items-center gap-3 py-2">
        <div className="scrollbar-hide flex flex-1 items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => {
            const isActive = tab.id === active;
            return (
              <button
                key={tab.id}
                type="button"
                aria-current={isActive ? "page" : undefined}
                onClick={() => onChange(tab.id)}
                className={`flex shrink-0 items-center gap-2 whitespace-nowrap rounded-lg px-3 py-1.5 text-sm transition-colors ${
                  isActive
                    ? "bg-primary/10 font-semibold text-primary ring-1 ring-inset ring-primary/25"
                    : "font-medium text-muted-foreground hover:bg-muted/60 hover:text-foreground"
                }`}
              >
                {tab.icon && (
                  <Icon
                    name={tab.icon}
                    size="sm"
                    mono
                    className={isActive ? "text-primary" : "text-muted-foreground/70"}
                  />
                )}
                {tab.label}
              </button>
            );
          })}
        </div>
        {trailing != null && <div className="hidden shrink-0 items-center sm:flex">{trailing}</div>}
      </div>
    </nav>
  );
}

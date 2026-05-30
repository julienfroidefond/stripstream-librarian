"use client";

import Link from "next/link";
import type { Route } from "next";
import { useTranslation } from "@/lib/i18n/context";

export function GroupByToggle({ active, href }: { active: boolean; href: Route }) {
  const { t } = useTranslation();

  return (
    <Link
      href={href}
      title={active ? t("series.groupByReadingListOff") : t("series.groupByReadingList")}
      className={`flex items-center gap-2 px-3 h-9 rounded-md border text-xs font-medium transition-colors ${
        active
          ? "border-cyan-500 bg-cyan-500/10 text-cyan-600 dark:text-cyan-400"
          : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
      }`}
    >
      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
      </svg>
      {t("series.groupByReadingList")}
    </Link>
  );
}

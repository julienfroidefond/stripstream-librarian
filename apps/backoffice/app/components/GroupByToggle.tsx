"use client";

import { useSearchParams, useRouter } from "next/navigation";
import { useTranslation } from "@/lib/i18n/context";

export function GroupByToggle({ active }: { active: boolean }) {
  const { t } = useTranslation();
  const router = useRouter();
  const searchParams = useSearchParams();

  function toggle() {
    const params = new URLSearchParams(searchParams.toString());
    if (active) {
      params.delete("group_by");
    } else {
      params.set("group_by", "reading_list");
    }
    router.push(`/series?${params.toString()}`);
  }

  return (
    <button
      type="button"
      onClick={toggle}
      title={active ? t("series.groupByReadingListOff") : t("series.groupByReadingList")}
      className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm font-medium transition-colors ${
        active
          ? "border-cyan-500 bg-cyan-500/10 text-cyan-600 dark:text-cyan-400"
          : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
      }`}
    >
      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
      </svg>
      {t("series.groupByReadingList")}
    </button>
  );
}

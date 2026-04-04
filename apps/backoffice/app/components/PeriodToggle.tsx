"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useTransition } from "react";

type Period = "day" | "week" | "month";

export function PeriodToggle({
  labels,
}: {
  labels: { day: string; week: string; month: string };
}) {
  const router = useRouter();
  const [, startTransition] = useTransition();
  const searchParams = useSearchParams();
  const raw = searchParams.get("period");
  const current: Period = raw === "day" ? "day" : raw === "month" ? "month" : "week";

  function setPeriod(period: Period) {
    const params = new URLSearchParams(searchParams.toString());
    if (period === "week") {
      params.delete("period");
    } else {
      params.set("period", period);
    }
    const qs = params.toString();
    const url = qs ? `?${qs}` : "/";
    window.history.replaceState(null, "", url);
    startTransition(() => { router.refresh(); });
  }

  const options: Period[] = ["day", "week", "month"];

  return (
    <div className="flex gap-1 bg-muted rounded-lg p-0.5">
      {options.map((p) => (
        <button
          key={p}
          onClick={() => setPeriod(p)}
          className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
            current === p
              ? "bg-card text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          {labels[p]}
        </button>
      ))}
    </div>
  );
}

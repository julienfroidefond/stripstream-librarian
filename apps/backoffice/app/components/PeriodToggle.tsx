"use client";

import { useRouter, useSearchParams } from "next/navigation";

type Period = "day" | "week" | "month";

export function PeriodToggle({
  labels,
}: {
  labels: { day: string; week: string; month: string };
}) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const raw = searchParams.get("period");
  const current: Period = raw === "day" ? "day" : raw === "week" ? "week" : "month";

  function setPeriod(period: Period) {
    const params = new URLSearchParams(searchParams.toString());
    if (period === "month") {
      params.delete("period");
    } else {
      params.set("period", period);
    }
    const qs = params.toString();
    router.push(qs ? `?${qs}` : "/", { scroll: false });
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

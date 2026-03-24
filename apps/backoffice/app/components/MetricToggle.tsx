"use client";

import { useRouter, useSearchParams } from "next/navigation";

type Metric = "books" | "pages";

export function MetricToggle({
  labels,
}: {
  labels: { books: string; pages: string };
}) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const raw = searchParams.get("metric");
  const current: Metric = raw === "pages" ? "pages" : "books";

  function setMetric(metric: Metric) {
    const params = new URLSearchParams(searchParams.toString());
    if (metric === "books") {
      params.delete("metric");
    } else {
      params.set("metric", metric);
    }
    const qs = params.toString();
    router.push(qs ? `?${qs}` : "/", { scroll: false });
  }

  const options: Metric[] = ["books", "pages"];

  return (
    <div className="flex gap-1 bg-muted rounded-lg p-0.5">
      {options.map((m) => (
        <button
          key={m}
          onClick={() => setMetric(m)}
          className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
            current === m
              ? "bg-card text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          {labels[m]}
        </button>
      ))}
    </div>
  );
}

"use client";

import { useState, useEffect, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcAreaChart, RcMultiLineChart } from "@/app/components/DashboardCharts";
import type {
  StatsResponse,
  MonthlyReading,
  UserMonthlyReading,
  MonthlyAdditions,
  JobTimePoint,
} from "@/lib/api";

type Period = "day" | "week" | "month";
type Metric = "books" | "pages";

function formatChartLabel(raw: string, period: Period, locale: string): string {
  const loc = locale === "fr" ? "fr-FR" : "en-US";
  if (period === "month") {
    const [y, m] = raw.split("-");
    const d = new Date(Number(y), Number(m) - 1, 1);
    return d.toLocaleDateString(loc, { month: "short" });
  }
  if (period === "week") {
    const d = new Date(raw + "T00:00:00");
    return d.toLocaleDateString(loc, { day: "numeric", month: "short" });
  }
  const d = new Date(raw + "T00:00:00");
  return d.toLocaleDateString(loc, { weekday: "short", day: "numeric" });
}

function PeriodToggle({
  labels,
  value,
  onChange,
}: {
  labels: { day: string; week: string; month: string };
  value: Period;
  onChange: (p: Period) => void;
}) {
  const options: Period[] = ["day", "week", "month"];
  return (
    <div className="flex gap-1 bg-muted rounded-lg p-0.5">
      {options.map((p) => (
        <button
          key={p}
          onClick={() => onChange(p)}
          className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
            value === p
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

function MetricToggle({
  labels,
  value,
  onChange,
}: {
  labels: { books: string; pages: string };
  value: Metric;
  onChange: (m: Metric) => void;
}) {
  const options: Metric[] = ["books", "pages"];
  return (
    <div className="flex gap-1 bg-muted rounded-lg p-0.5">
      {options.map((m) => (
        <button
          key={m}
          onClick={() => onChange(m)}
          className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors ${
            value === m
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

type TimeSeriesData = {
  reading_over_time: MonthlyReading[];
  users_reading_over_time: UserMonthlyReading[];
  additions_over_time: MonthlyAdditions[];
  jobs_over_time: JobTimePoint[];
};

export function TimeSeriesCharts({
  initialData,
  locale,
  labels,
}: {
  initialData: TimeSeriesData;
  locale: string;
  labels: {
    readingActivity: string;
    booksAdded: string;
    jobsOverTime: string;
    periodDay: string;
    periodWeek: string;
    periodMonth: string;
    metricBooks: string;
    metricPages: string;
    noData: string;
    jobScan: string;
    jobRebuild: string;
    jobThumbnail: string;
    jobOther: string;
  };
}) {
  const [period, setPeriod] = useState<Period>("week");
  const [metric, setMetric] = useState<Metric>("books");
  const [data, setData] = useState<TimeSeriesData>(initialData);
  const [loading, setLoading] = useState(false);

  const fetchData = useCallback(async (p: Period) => {
    setLoading(true);
    try {
      const params = p !== "week" ? `?period=${p}` : "";
      const res = await fetch(`/api/stats${params}`);
      if (res.ok) {
        const stats: StatsResponse = await res.json();
        setData({
          reading_over_time: stats.reading_over_time ?? [],
          users_reading_over_time: stats.users_reading_over_time ?? [],
          additions_over_time: stats.additions_over_time ?? [],
          jobs_over_time: stats.jobs_over_time ?? [],
        });
      }
    } catch {
      // keep existing data on error
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // Skip initial fetch — we already have server-rendered data for "week"
    if (period === "week") return;
    fetchData(period);
  }, [period, fetchData]);

  function handlePeriodChange(p: Period) {
    if (p === period) return;
    if (p === "week") {
      // Reset to initial server data
      setData(initialData);
    }
    setPeriod(p);
  }

  const userColors = [
    "hsl(142 60% 45%)", "hsl(198 78% 37%)", "hsl(45 93% 47%)",
    "hsl(2 72% 48%)", "hsl(280 60% 50%)", "hsl(32 80% 50%)",
  ];
  const dataKey = metric === "pages" ? "pages_read" : "books_read";

  const periodLabels = { day: labels.periodDay, week: labels.periodWeek, month: labels.periodMonth };
  const metricLabels = { books: labels.metricBooks, pages: labels.metricPages };

  const opacity = loading ? "opacity-50 transition-opacity" : "transition-opacity";

  return (
    <>
      {/* Reading activity */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{labels.readingActivity}</CardTitle>
          <div className="flex flex-wrap gap-1.5 justify-end">
            <MetricToggle labels={metricLabels} value={metric} onChange={setMetric} />
            <PeriodToggle labels={periodLabels} value={period} onChange={handlePeriodChange} />
          </div>
        </CardHeader>
        <CardContent className={opacity}>
          {(() => {
            const usernames = [...new Set(data.users_reading_over_time.map(r => r.username))];
            if (usernames.length === 0) {
              return (
                <RcAreaChart
                  noDataLabel={labels.noData}
                  data={data.reading_over_time.map((m) => ({ label: formatChartLabel(m.month, period, locale), value: m[dataKey] }))}
                  color="hsl(142 60% 45%)"
                />
              );
            }
            const byMonth = new Map<string, Record<string, unknown>>();
            for (const row of data.users_reading_over_time) {
              const label = formatChartLabel(row.month, period, locale);
              if (!byMonth.has(row.month)) byMonth.set(row.month, { label });
              byMonth.get(row.month)![row.username] = row[dataKey];
            }
            const chartData = [...byMonth.values()];
            const lines = usernames.map((u, i) => ({
              key: u,
              label: u,
              color: userColors[i % userColors.length],
            }));
            return <RcMultiLineChart data={chartData} lines={lines} noDataLabel={labels.noData} />;
          })()}
        </CardContent>
      </Card>

      {/* Books added */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{labels.booksAdded}</CardTitle>
          <PeriodToggle labels={periodLabels} value={period} onChange={handlePeriodChange} />
        </CardHeader>
        <CardContent className={opacity}>
          <RcAreaChart
            noDataLabel={labels.noData}
            data={data.additions_over_time.map((m) => ({ label: formatChartLabel(m.month, period, locale), value: m.books_added }))}
            color="hsl(198 78% 37%)"
          />
        </CardContent>
      </Card>

      {/* Jobs over time */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{labels.jobsOverTime}</CardTitle>
          <PeriodToggle labels={periodLabels} value={period} onChange={handlePeriodChange} />
        </CardHeader>
        <CardContent className={opacity}>
          <RcMultiLineChart
            noDataLabel={labels.noData}
            data={data.jobs_over_time.map((j) => ({
              label: formatChartLabel(j.label, period, locale),
              scan: j.scan,
              rebuild: j.rebuild,
              thumbnail: j.thumbnail,
              other: j.other,
            }))}
            lines={[
              { key: "scan", label: labels.jobScan, color: "hsl(198 78% 37%)" },
              { key: "rebuild", label: labels.jobRebuild, color: "hsl(142 60% 45%)" },
              { key: "thumbnail", label: labels.jobThumbnail, color: "hsl(45 93% 47%)" },
              { key: "other", label: labels.jobOther, color: "hsl(280 60% 50%)" },
            ]}
          />
        </CardContent>
      </Card>
    </>
  );
}

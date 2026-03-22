"use client";

import {
  PieChart, Pie, Cell, ResponsiveContainer, Tooltip,
  BarChart, Bar, XAxis, YAxis, CartesianGrid,
  AreaChart, Area, Line, LineChart,
  Legend,
} from "recharts";

// ---------------------------------------------------------------------------
// Donut
// ---------------------------------------------------------------------------

export function RcDonutChart({
  data,
  noDataLabel,
}: {
  data: { name: string; value: number; color: string }[];
  noDataLabel?: string;
}) {
  const total = data.reduce((s, d) => s + d.value, 0);
  if (total === 0) return <p className="text-muted-foreground text-sm text-center py-8">{noDataLabel}</p>;

  return (
    <div className="flex items-center gap-4">
      <ResponsiveContainer width={130} height={130}>
        <PieChart>
          <Pie
            data={data}
            cx="50%"
            cy="50%"
            innerRadius={32}
            outerRadius={55}
            dataKey="value"
            strokeWidth={0}
          >
            {data.map((d, i) => (
              <Cell key={i} fill={d.color} />
            ))}
          </Pie>
          <Tooltip
            formatter={(value) => value}
            contentStyle={{ backgroundColor: "var(--color-card)", border: "1px solid var(--color-border)", borderRadius: 8, fontSize: 12 }}
          />
        </PieChart>
      </ResponsiveContainer>
      <div className="flex flex-col gap-1.5 min-w-0">
        {data.map((d, i) => (
          <div key={i} className="flex items-center gap-2 text-sm">
            <span className="w-3 h-3 rounded-full shrink-0" style={{ backgroundColor: d.color }} />
            <span className="text-muted-foreground truncate">{d.name}</span>
            <span className="font-medium text-foreground ml-auto">{d.value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Bar chart
// ---------------------------------------------------------------------------

export function RcBarChart({
  data,
  color = "hsl(198 78% 37%)",
  noDataLabel,
}: {
  data: { label: string; value: number }[];
  color?: string;
  noDataLabel?: string;
}) {
  if (data.length === 0) return <p className="text-muted-foreground text-sm text-center py-8">{noDataLabel}</p>;

  return (
    <ResponsiveContainer width="100%" height={180}>
      <BarChart data={data} margin={{ top: 5, right: 5, bottom: 0, left: -20 }}>
        <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="var(--color-border)" opacity={0.3} />
        <XAxis dataKey="label" tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} />
        <YAxis tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} allowDecimals={false} />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--color-card)", border: "1px solid var(--color-border)", borderRadius: 8, fontSize: 12 }}
        />
        <Bar dataKey="value" fill={color} radius={[4, 4, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
}

// ---------------------------------------------------------------------------
// Area / Line chart
// ---------------------------------------------------------------------------

export function RcAreaChart({
  data,
  color = "hsl(142 60% 45%)",
  noDataLabel,
}: {
  data: { label: string; value: number }[];
  color?: string;
  noDataLabel?: string;
}) {
  if (data.length === 0) return <p className="text-muted-foreground text-sm text-center py-8">{noDataLabel}</p>;

  return (
    <ResponsiveContainer width="100%" height={180}>
      <AreaChart data={data} margin={{ top: 5, right: 5, bottom: 0, left: -20 }}>
        <defs>
          <linearGradient id="areaGradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity={0.3} />
            <stop offset="100%" stopColor={color} stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="var(--color-border)" opacity={0.3} />
        <XAxis dataKey="label" tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} />
        <YAxis tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} allowDecimals={false} />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--color-card)", border: "1px solid var(--color-border)", borderRadius: 8, fontSize: 12 }}
        />
        <Area type="monotone" dataKey="value" stroke={color} strokeWidth={2} fill="url(#areaGradient)" dot={{ r: 3, fill: color }} />
      </AreaChart>
    </ResponsiveContainer>
  );
}

// ---------------------------------------------------------------------------
// Horizontal stacked bar (libraries breakdown)
// ---------------------------------------------------------------------------

export function RcStackedBar({
  data,
  labels,
}: {
  data: { name: string; read: number; reading: number; unread: number; sizeLabel: string }[];
  labels: { read: string; reading: string; unread: string; books: string };
}) {
  if (data.length === 0) return null;

  return (
    <ResponsiveContainer width="100%" height={data.length * 60 + 30}>
      <BarChart data={data} layout="vertical" margin={{ top: 0, right: 5, bottom: 0, left: 5 }}>
        <CartesianGrid strokeDasharray="3 3" horizontal={false} stroke="var(--color-border)" opacity={0.3} />
        <XAxis type="number" tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} allowDecimals={false} />
        <YAxis type="category" dataKey="name" tick={{ fontSize: 12, fill: "var(--color-foreground)" }} axisLine={false} tickLine={false} width={120} />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--color-card)", border: "1px solid var(--color-border)", borderRadius: 8, fontSize: 12 }}
        />
        <Legend
          wrapperStyle={{ fontSize: 11 }}
          formatter={(value: string) => <span className="text-muted-foreground">{value}</span>}
        />
        <Bar dataKey="read" stackId="a" fill="hsl(142 60% 45%)" name={labels.read} radius={[0, 0, 0, 0]} />
        <Bar dataKey="reading" stackId="a" fill="hsl(45 93% 47%)" name={labels.reading} />
        <Bar dataKey="unread" stackId="a" fill="hsl(220 13% 70%)" name={labels.unread} radius={[0, 4, 4, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
}

// ---------------------------------------------------------------------------
// Horizontal bar chart (top series)
// ---------------------------------------------------------------------------

export function RcHorizontalBar({
  data,
  color = "hsl(142 60% 45%)",
  noDataLabel,
}: {
  data: { name: string; value: number; subLabel: string }[];
  color?: string;
  noDataLabel?: string;
}) {
  if (data.length === 0) return <p className="text-muted-foreground text-sm text-center py-4">{noDataLabel}</p>;

  return (
    <ResponsiveContainer width="100%" height={data.length * 40 + 10}>
      <BarChart data={data} layout="vertical" margin={{ top: 0, right: 5, bottom: 0, left: 5 }}>
        <CartesianGrid strokeDasharray="3 3" horizontal={false} stroke="var(--color-border)" opacity={0.3} />
        <XAxis type="number" tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} allowDecimals={false} />
        <YAxis type="category" dataKey="name" tick={{ fontSize: 11, fill: "var(--color-foreground)" }} axisLine={false} tickLine={false} width={120} />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--color-card)", border: "1px solid var(--color-border)", borderRadius: 8, fontSize: 12 }}
        />
        <Bar dataKey="value" fill={color} radius={[0, 4, 4, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
}

// ---------------------------------------------------------------------------
// Multi-line chart (jobs over time)
// ---------------------------------------------------------------------------

export function RcMultiLineChart({
  data,
  lines,
  noDataLabel,
}: {
  data: Record<string, unknown>[];
  lines: { key: string; label: string; color: string }[];
  noDataLabel?: string;
}) {
  const hasData = data.some((d) => lines.some((l) => (d[l.key] as number) > 0));
  if (data.length === 0 || !hasData)
    return <p className="text-muted-foreground text-sm text-center py-8">{noDataLabel}</p>;

  return (
    <ResponsiveContainer width="100%" height={180}>
      <LineChart data={data} margin={{ top: 5, right: 5, bottom: 0, left: -20 }}>
        <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="var(--color-border)" opacity={0.3} />
        <XAxis dataKey="label" tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} />
        <YAxis tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }} axisLine={false} tickLine={false} allowDecimals={false} />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--color-card)", border: "1px solid var(--color-border)", borderRadius: 8, fontSize: 12 }}
        />
        <Legend wrapperStyle={{ fontSize: 11 }} />
        {lines.map((l) => (
          <Line
            key={l.key}
            type="monotone"
            dataKey={l.key}
            name={l.label}
            stroke={l.color}
            strokeWidth={2}
            dot={{ r: 3, fill: l.color }}
          />
        ))}
      </LineChart>
    </ResponsiveContainer>
  );
}

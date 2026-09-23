import type { ReactNode } from "react";
import Image from "next/image";
import Link from "next/link";

import { getBookCoverUrl, type BookDto, type SeriesDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { Badge, Icon } from "@/app/components/ui";

const HEAD_CELL =
  "px-3 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground";
const BODY_CELL = "px-3 py-2 align-middle";
const EMPTY = "—";

const SERIES_STATUS_CLASSES: Record<string, string> = {
  ongoing: "bg-blue-500 text-white",
  ended: "bg-green-500 text-white",
  hiatus: "bg-amber-500 text-white",
  cancelled: "bg-red-500 text-white",
};

interface TableColumn {
  key: string;
  label: string;
}

function TableShell({ columns, children }: { columns: TableColumn[]; children: ReactNode }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[760px] text-sm">
        <thead className="bg-muted/50">
          <tr>
            {columns.map((column) => (
              <th key={column.key} className={HEAD_CELL}>
                {column.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-border/60">{children}</tbody>
      </table>
    </div>
  );
}

function Empty() {
  return <span className="text-muted-foreground">{EMPTY}</span>;
}

function SeriesStatusCell({
  status,
  knownStatuses,
}: {
  status: string | null;
  knownStatuses: Record<string, string>;
}) {
  if (!status) return <Empty />;
  const className = SERIES_STATUS_CLASSES[status] ?? "bg-muted text-muted-foreground";
  return (
    <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${className}`}>
      {knownStatuses[status] || status}
    </span>
  );
}

function GenresCell({ genres }: { genres: string[] }) {
  if (genres.length === 0) return <Empty />;
  const visible = genres.slice(0, 3);
  const remaining = genres.length - visible.length;
  return (
    <div className="flex flex-wrap items-center gap-1">
      {visible.map((genre) => (
        <span key={genre} className="rounded-full bg-success/10 px-1.5 py-0.5 text-xs text-success">
          {genre}
        </span>
      ))}
      {remaining > 0 && <span className="text-xs text-muted-foreground">+{remaining}</span>}
    </div>
  );
}

function RatingCell({
  userRating,
  communityScore,
}: {
  userRating: number | null;
  communityScore: number | null;
}) {
  if (userRating != null) return <span className="font-medium text-amber-500">★ {userRating}</span>;
  if (communityScore != null) {
    return <span className="text-muted-foreground">★ {communityScore.toFixed(1)}</span>;
  }
  return <Empty />;
}

function SeriesCoverCell({ series, t }: { series: SeriesDto; t: TranslateFunction }) {
  const src = series.first_book_id
    ? getBookCoverUrl(series.first_book_id, series.first_book_updated_at)
    : series.cover_url;
  if (!src) return <div className="h-12 w-9 rounded-md bg-muted" />;
  return (
    <Image
      src={src}
      alt={t("books.coverOf", { name: series.name })}
      width={36}
      height={54}
      className="h-12 w-9 rounded-md bg-muted object-cover"
    />
  );
}

interface MetadataSeriesTableProps {
  series: SeriesDto[];
  t: TranslateFunction;
  knownStatuses: Record<string, string>;
}

export function MetadataSeriesTable({ series, t, knownStatuses }: MetadataSeriesTableProps) {
  const columns: TableColumn[] = [
    { key: "cover", label: t("metadata.table.cover") },
    { key: "name", label: t("metadata.table.name") },
    { key: "provider", label: t("metadata.table.provider") },
    { key: "status", label: t("metadata.table.status") },
    { key: "year", label: t("metadata.table.year") },
    { key: "genres", label: t("metadata.table.genres") },
    { key: "volumes", label: t("metadata.table.volumes") },
    { key: "missing", label: t("metadata.table.missing") },
    { key: "rating", label: t("metadata.table.rating") },
  ];

  return (
    <TableShell columns={columns}>
      {series.map((s) => (
        <tr key={`${s.library_id}-${s.series_id}`} className="transition-colors hover:bg-accent/50">
          <td className={BODY_CELL}>
            <SeriesCoverCell series={s} t={t} />
          </td>
          <td className={BODY_CELL}>
            <Link
              href={`/series/${s.series_id}`}
              className="font-medium text-foreground transition-colors hover:text-primary"
            >
              {s.name === "unclassified" ? t("books.unclassified") : s.name}
            </Link>
          </td>
          <td className={BODY_CELL}>
            {s.metadata_provider ? <Badge variant="muted">{s.metadata_provider}</Badge> : <Empty />}
          </td>
          <td className={BODY_CELL}>
            <SeriesStatusCell status={s.series_status} knownStatuses={knownStatuses} />
          </td>
          <td className={BODY_CELL}>{s.start_year ?? <Empty />}</td>
          <td className={BODY_CELL}>
            <GenresCell genres={s.genres} />
          </td>
          <td className={BODY_CELL}>{s.book_count}</td>
          <td className={BODY_CELL}>
            {s.missing_count != null && s.missing_count > 0 ? (
              <Badge variant="warning">{s.missing_count}</Badge>
            ) : (
              <Empty />
            )}
          </td>
          <td className={BODY_CELL}>
            <RatingCell userRating={s.user_rating} communityScore={s.community_score} />
          </td>
        </tr>
      ))}
    </TableShell>
  );
}

interface MetadataBooksTableProps {
  books: BookDto[];
  t: TranslateFunction;
}

export function MetadataBooksTable({ books, t }: MetadataBooksTableProps) {
  const columns: TableColumn[] = [
    { key: "title", label: t("metadata.table.title") },
    { key: "series", label: t("metadata.table.series") },
    { key: "volume", label: t("metadata.table.volume") },
    { key: "authors", label: t("metadata.table.authors") },
    { key: "language", label: t("metadata.table.language") },
    { key: "pages", label: t("metadata.table.pages") },
    { key: "format", label: t("metadata.table.format") },
    { key: "summary", label: t("metadata.table.summary") },
    { key: "isbn", label: t("metadata.table.isbn") },
  ];

  return (
    <TableShell columns={columns}>
      {books.map((b) => {
        const authors = b.authors.length > 0 ? b.authors.join(", ") : b.author;
        const format = b.format ?? b.file_format;
        return (
          <tr key={b.id} className="transition-colors hover:bg-accent/50">
            <td className={BODY_CELL}>
              <Link
                href={`/books/${b.id}`}
                className="font-medium text-foreground transition-colors hover:text-primary"
              >
                {b.title}
              </Link>
            </td>
            <td className={BODY_CELL}>{b.series ?? t("books.unclassified")}</td>
            <td className={BODY_CELL}>{b.volume ?? <Empty />}</td>
            <td className={BODY_CELL}>
              {authors ? <span className="text-muted-foreground">{authors}</span> : <Empty />}
            </td>
            <td className={BODY_CELL}>
              {b.language ? <span className="uppercase text-muted-foreground">{b.language}</span> : <Empty />}
            </td>
            <td className={BODY_CELL}>{b.page_count ?? <Empty />}</td>
            <td className={BODY_CELL}>
              {format ? <span className="uppercase text-muted-foreground">{format}</span> : <Empty />}
            </td>
            <td className={BODY_CELL}>
              {b.summary ? (
                <Icon name="check" size="sm" className="text-success" />
              ) : (
                <Badge variant="muted">{t("metadata.gap.noSummary")}</Badge>
              )}
            </td>
            <td className={BODY_CELL}>{b.isbn ?? <Empty />}</td>
          </tr>
        );
      })}
    </TableShell>
  );
}

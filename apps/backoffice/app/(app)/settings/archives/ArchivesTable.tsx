"use client";

import React, { useState } from "react";
import { ArchivedSeriesItemDto, ArchivedSeriesDetailDto, ArchivedBookReadingProgressDto, ArchivedSeriesReadingProgressDto } from "@/lib/api";
import { Badge } from "@/app/components/ui/Badge";
import { Icon } from "@/app/components/ui/Icon";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

function volumeLabel(t: TranslateFunction, volume: number | null, volumeType: string): string {
  if (volumeType === "hs") return volume != null ? t("settings.archives.volumeHs", { volume }) : t("volumeType.hs");
  if (volumeType === "oneshot") return t("volumeType.oneshot");
  if (volumeType === "integral") return t("volumeType.integral");
  return volume != null ? t("settings.archives.volume", { volume }) : "?";
}

function SeriesReadingProgress({ progress, bookCount }: { progress: ArchivedSeriesReadingProgressDto[]; bookCount: number }) {
  const { t } = useTranslation();
  if (progress.length === 0) return <span className="text-muted-foreground text-xs">—</span>;
  return (
    <div className="flex flex-wrap gap-1.5">
      {progress.map((p) => {
        const allRead = bookCount > 0 && p.books_read >= bookCount;
        const variant = allRead ? "success" : p.books_reading > 0 || p.books_read > 0 ? "warning" : "secondary";
        return (
          <span key={p.user_name} className="inline-flex items-center gap-1">
            <Badge variant={variant}>{p.books_read}/{bookCount}</Badge>
            <span className="text-xs text-muted-foreground">{p.user_name}</span>
          </span>
        );
      })}
    </div>
  );
}

function ReadingProgressBadges({ progress }: { progress: ArchivedBookReadingProgressDto[] }) {
  const { t } = useTranslation();
  if (progress.length === 0) return <span className="text-muted-foreground text-xs">—</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {progress.map((p) => {
        const variant =
          p.status === "read" ? "success" :
          p.status === "reading" ? "warning" :
          "secondary";
        const label =
          p.status === "read" ? t("status.read") :
          p.status === "reading" ? `${t("status.reading")}${p.current_page ? ` · p.${p.current_page}` : ""}` :
          t("status.unread");
        return (
          <span key={p.user_name} className="inline-flex items-center gap-1">
            <Badge variant={variant}>{label}</Badge>
            <span className="text-xs text-muted-foreground">{p.user_name}</span>
          </span>
        );
      })}
    </div>
  );
}

interface Props {
  series: ArchivedSeriesItemDto[];
}

export function ArchivesTable({ series }: Props) {
  const { t, locale } = useTranslation();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [details, setDetails] = useState<Record<string, ArchivedSeriesDetailDto>>({});
  const [loading, setLoading] = useState<Record<string, boolean>>({});

  async function toggle(id: string) {
    if (expandedId === id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(id);
    if (!details[id]) {
      setLoading((l) => ({ ...l, [id]: true }));
      const detail = await fetch(`/api/series/archived/${id}`)
        .then((r) => r.ok ? r.json() as Promise<ArchivedSeriesDetailDto> : null)
        .catch(() => null);
      if (detail) setDetails((d) => ({ ...d, [id]: detail }));
      setLoading((l) => ({ ...l, [id]: false }));
    }
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-muted-foreground">
            <th className="w-8 px-4 py-3" />
            <th className="text-left px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("settings.archives.colSeries")}</th>
            <th className="text-left px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden sm:table-cell">{t("settings.archives.colAuthors")}</th>
            <th className="text-left px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden lg:table-cell">{t("settings.archives.colPublisher")}</th>
            <th className="text-left px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden md:table-cell">{t("settings.archives.colGenres")}</th>
            <th className="text-right px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden sm:table-cell">{t("settings.archives.colYear")}</th>
            <th className="text-right px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("settings.archives.colVolumes")}</th>
            <th className="text-left px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden sm:table-cell">{t("settings.archives.colReading")}</th>
            <th className="text-right px-4 py-3 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden sm:table-cell">{t("settings.archives.colArchivedAt")}</th>
          </tr>
        </thead>
        <tbody>
          {series.map((s) => {
            const isExpanded = expandedId === s.id;
            const isLoading = loading[s.id];
            const detail = details[s.id];

            return (
              <React.Fragment key={s.id}>
                <tr
                  className="border-b border-border/50 hover:bg-muted/40 transition-colors cursor-pointer"
                  onClick={() => toggle(s.id)}
                >
                  <td className="px-4 py-3 text-muted-foreground">
                    <Icon
                      name="chevronRight"
                      size="sm"
                      className={`transition-transform ${isExpanded ? "rotate-90" : ""}`}
                    />
                  </td>
                  <td className="px-4 py-3">
                    <span className="font-medium text-foreground">{s.name}</span>
                    {s.status && (
                      <span className="ml-2 text-xs text-muted-foreground">
                        {t(`seriesStatus.${s.status}` as Parameters<typeof t>[0])}
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-muted-foreground hidden sm:table-cell">
                    {s.authors.length > 0 ? s.authors.join(", ") : "—"}
                  </td>
                  <td className="px-4 py-3 text-muted-foreground hidden lg:table-cell">
                    {s.publishers.length > 0 ? s.publishers[0] : "—"}
                  </td>
                  <td className="px-4 py-3 hidden md:table-cell">
                    <div className="flex flex-wrap gap-1">
                      {s.genres.slice(0, 3).map((g) => (
                        <Badge key={g} variant="secondary">{g}</Badge>
                      ))}
                      {s.genres.length > 3 && (
                        <span className="text-xs text-muted-foreground">+{s.genres.length - 3}</span>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right text-muted-foreground tabular-nums hidden sm:table-cell">
                    {s.start_year ?? "—"}
                  </td>
                  <td className="px-4 py-3 text-right text-muted-foreground tabular-nums">
                    {s.book_count}{s.total_volumes ? ` / ${s.total_volumes}` : ""}
                  </td>
                  <td className="px-4 py-3 hidden sm:table-cell">
                    <SeriesReadingProgress progress={s.reading_progress} bookCount={Number(s.book_count)} />
                  </td>
                  <td className="px-4 py-3 text-right text-muted-foreground hidden sm:table-cell whitespace-nowrap">
                    {new Date(s.archived_at).toLocaleDateString(locale)}
                  </td>
                </tr>

                {isExpanded && (
                  <tr key={`${s.id}-detail`} className="border-b border-border/50 bg-muted/20">
                    <td />
                    <td colSpan={8} className="px-4 py-4">
                      {isLoading ? (
                        <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
                      ) : !detail ? (
                        <p className="text-sm text-muted-foreground">{t("settings.archives.detailError")}</p>
                      ) : (
                        <div className="space-y-4">
                          {detail.description && (
                            <p className="text-sm text-muted-foreground line-clamp-3">{detail.description}</p>
                          )}

                          {detail.books.length === 0 ? (
                            <p className="text-sm text-muted-foreground italic">{t("settings.archives.noBooks")}</p>
                          ) : (
                            <table className="w-full text-sm">
                              <thead>
                                <tr className="text-muted-foreground border-b border-border/50">
                                  <th className="text-left py-1.5 pr-4 text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("settings.archives.colVolume")}</th>
                                  <th className="text-left py-1.5 pr-4 text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("settings.archives.colTitle")}</th>
                                  <th className="text-left py-1.5 pr-4 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden md:table-cell">{t("settings.archives.colAuthor")}</th>
                                  <th className="text-right py-1.5 pr-4 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden sm:table-cell">{t("settings.archives.colPages")}</th>
                                  <th className="text-left py-1.5 pr-4 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden md:table-cell">{t("settings.archives.colFormat")}</th>
                                  <th className="text-left py-1.5 pr-4 text-xs font-semibold text-muted-foreground uppercase tracking-wider hidden lg:table-cell">{t("settings.archives.colPublication")}</th>
                                  <th className="text-left py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("settings.archives.colReading")}</th>
                                </tr>
                              </thead>
                              <tbody>
                                {detail.books.map((book) => (
                                  <tr key={book.id} className="border-b border-border/30 last:border-0">
                                    <td className="py-1.5 pr-4 font-medium text-foreground whitespace-nowrap">
                                      {volumeLabel(t, book.volume, book.volume_type)}
                                    </td>
                                    <td className="py-1.5 pr-4 text-muted-foreground">{book.title}</td>
                                    <td className="py-1.5 pr-4 text-muted-foreground hidden md:table-cell">
                                      {book.author ?? "—"}
                                    </td>
                                    <td className="py-1.5 pr-4 text-right text-muted-foreground tabular-nums hidden sm:table-cell">
                                      {book.page_count ?? "—"}
                                    </td>
                                    <td className="py-1.5 pr-4 text-muted-foreground uppercase text-xs hidden md:table-cell">
                                      {book.format ?? "—"}
                                    </td>
                                    <td className="py-1.5 pr-4 text-muted-foreground hidden lg:table-cell">
                                      {book.publish_date ?? "—"}
                                    </td>
                                    <td className="py-1.5">
                                      <ReadingProgressBadges progress={book.reading_progress} />
                                    </td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          )}

                        </div>
                      )}
                    </td>
                  </tr>
                )}
              </React.Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

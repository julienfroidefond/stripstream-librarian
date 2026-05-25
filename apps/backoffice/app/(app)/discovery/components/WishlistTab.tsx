"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { getBookCoverUrl, type SeriesDto } from "@/lib/api";

interface Library {
  id: string;
  name: string;
}

export function WishlistTab({ libraries }: { libraries: Library[] }) {
  const { t } = useTranslation();
  const [series, setSeries] = useState<SeriesDto[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function fetchWishlist() {
      setLoading(true);
      try {
        const resp = await fetch("/api/series?no_books=true&limit=100");
        if (resp.ok) {
          const data = await resp.json();
          setSeries(data.items ?? []);
        }
      } catch { /* ignore */ } finally {
        setLoading(false);
      }
    }
    fetchWishlist();
  }, []);

  const libraryName = (id: string) => libraries.find((l) => l.id === id)?.name ?? "";

  if (loading) {
    return (
      <div className="flex justify-center py-12">
        <Icon name="spinner" size="lg" className="animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (series.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center gap-3">
        <Icon name="series" size="xl" className="text-muted-foreground/30" />
        <p className="text-muted-foreground">{t("discovery.wishlistEmpty")}</p>
        <p className="text-sm text-muted-foreground/60">{t("discovery.wishlistEmptyHint")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        {t("discovery.wishlistCount", { count: String(series.length), plural: series.length !== 1 ? "s" : "" })}
      </p>
      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
        {series.map((s) => (
          <Link key={s.series_id} href={`/series/${s.series_id}`} className="group block">
            <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden group-hover:shadow-md group-hover:-translate-y-1 transition-all duration-200">
              <div className="aspect-[2/3] relative bg-muted/50">
                {(s.first_book_id || s.cover_url) ? (
                  <img
                    src={s.first_book_id ? getBookCoverUrl(s.first_book_id, s.first_book_updated_at) : s.cover_url!}
                    alt={s.name}
                    className="w-full h-full object-cover"
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-muted-foreground/30">
                    <Icon name="books" size="xl" />
                  </div>
                )}
                <div className="absolute top-1.5 right-1.5 flex flex-col items-end gap-1">
                  {s.missing_count != null && s.missing_count > 0 && (
                    <span className="inline-flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded-full font-bold bg-yellow-500 text-white">
                      <svg className="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5}>
                        <path d="M12 9v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                      {s.missing_count}
                    </span>
                  )}
                  {s.series_status && (
                    <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                      s.series_status === "ongoing" ? "bg-blue-500 text-white" :
                      s.series_status === "ended" ? "bg-green-500 text-white" :
                      s.series_status === "hiatus" ? "bg-amber-500 text-white" :
                      s.series_status === "cancelled" ? "bg-red-500 text-white" :
                      "bg-muted text-muted-foreground"
                    }`}>
                      {t(`seriesStatus.${s.series_status}` as Parameters<typeof t>[0]) || s.series_status}
                    </span>
                  )}
                </div>
              </div>
              <div className="px-2 py-1.5">
                <h3 className="font-medium text-foreground truncate text-xs group-hover:text-primary transition-colors" title={s.name}>
                  {s.name}
                </h3>
                <p className="text-[11px] text-muted-foreground truncate mt-0.5">
                  {libraryName(s.library_id)}
                </p>
              </div>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}

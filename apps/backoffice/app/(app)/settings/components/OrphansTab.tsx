"use client";

import { useCallback, useEffect, useState } from "react";
import { Button, Icon, SettingsCard } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { OrphanSeriesPageDto } from "@/lib/api";
import { OrphansTable } from "../orphans/OrphansTable";

const PAGE_SIZE = 50;

export function OrphansTab() {
  const { t } = useTranslation();
  const [page, setPage] = useState(1);
  const [data, setData] = useState<OrphanSeriesPageDto | null>(null);
  const [failed, setFailed] = useState(false);
  // Bumped after a deletion so the current page is refetched without losing it.
  const [reload, setReload] = useState(0);

  useEffect(() => {
    let active = true;
    fetch(`/api/series/orphans?page=${page}&limit=${PAGE_SIZE}`)
      .then((r) => {
        if (!r.ok) throw new Error("load failed");
        return r.json() as Promise<OrphanSeriesPageDto>;
      })
      .then((payload) => {
        if (!active) return;
        setFailed(false);
        setData(payload);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [page, reload]);

  const handleDeleted = useCallback(
    (ids: string[]) => {
      // Deleting the last items of a page would leave the user on an empty page.
      if (data && page > 1 && ids.length >= data.items.length) {
        setPage(page - 1);
      } else {
        setReload((value) => value + 1);
      }
    },
    [data, page],
  );

  if (failed) {
    return <p className="text-destructive text-sm">{t("settings.orphans.loadError")}</p>;
  }

  if (!data) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground text-sm">
        <Icon name="spinner" size="sm" className="animate-spin" />
        {t("common.loading")}
      </div>
    );
  }

  if (data.total === 0) {
    return (
      <SettingsCard
        icon="books"
        title={t("settings.orphans")}
        description={t("settings.orphans.desc")}
      >
        <div className="py-10 text-center">
          <Icon name="books" size="lg" mono className="mx-auto mb-3 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t("settings.orphans.empty")}</p>
          <p className="mt-1 text-xs text-muted-foreground">{t("settings.orphans.emptyHint")}</p>
        </div>
      </SettingsCard>
    );
  }

  const totalPages = Math.max(1, Math.ceil(data.total / data.limit));

  return (
    <SettingsCard
      icon="books"
      title={t("settings.orphans")}
      description={t("settings.orphans.count", {
        count: data.total,
        plural: data.total !== 1 ? "s" : "",
      })}
      bodyClassName="p-0"
    >
      <OrphansTable series={data.items} onDeleted={handleDeleted} />

      {totalPages > 1 && (
        <div className="flex items-center justify-between gap-2 border-t border-border/50 px-4 py-3">
          <span className="text-xs text-muted-foreground">
            {t("settings.orphans.pageInfo", { page, pages: totalPages })}
          </span>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((value) => Math.max(1, value - 1))}
              disabled={page <= 1}
            >
              {t("common.previous")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((value) => Math.min(totalPages, value + 1))}
              disabled={page >= totalPages}
            >
              {t("common.next")}
            </Button>
          </div>
        </div>
      )}
    </SettingsCard>
  );
}

"use client";

import { useEffect, useState } from "react";
import { Card, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { ArchivedSeriesItemDto } from "@/lib/api";
import { ArchivesTable } from "../archives/ArchivesTable";

export function ArchivesTab() {
  const { t } = useTranslation();
  const [series, setSeries] = useState<ArchivedSeriesItemDto[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    fetch("/api/series/archived")
      .then((r) => r.json())
      .then((data) => setSeries(Array.isArray(data) ? data : []))
      .catch(() => setFailed(true));
  }, []);

  if (failed) {
    return <p className="text-destructive text-sm">{t("settings.archives.loadError")}</p>;
  }

  if (!series) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground text-sm">
        <Icon name="spinner" size="sm" className="animate-spin" />
        {t("common.loading")}
      </div>
    );
  }

  if (series.length === 0) {
    return (
      <Card>
        <div className="py-12 text-center">
          <Icon name="books" size="lg" className="text-muted-foreground mx-auto mb-3" />
          <p className="text-muted-foreground">{t("settings.archives.empty")}</p>
          <p className="text-sm text-muted-foreground mt-1">
            {t("settings.archives.emptyHint")}
          </p>
        </div>
      </Card>
    );
  }

  return (
    <div>
      <p className="text-sm text-muted-foreground mb-4">
        {t("settings.archives.count", {
          count: series.length,
          plural: series.length !== 1 ? "s" : "",
        })}
      </p>
      <Card>
        <ArchivesTable series={series} />
      </Card>
    </div>
  );
}

"use client";

import { useEffect, useState } from "react";
import { SettingsCard, Icon } from "@/app/components/ui";
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
      <SettingsCard icon="books" title={t("settings.archives")} description={t("settings.archives.desc")}>
        <div className="py-10 text-center">
          <Icon name="books" size="lg" mono className="mx-auto mb-3 text-muted-foreground" />
          <p className="text-sm text-muted-foreground">{t("settings.archives.empty")}</p>
          <p className="mt-1 text-xs text-muted-foreground">
            {t("settings.archives.emptyHint")}
          </p>
        </div>
      </SettingsCard>
    );
  }

  return (
    <SettingsCard
      icon="books"
      title={t("settings.archives")}
      description={t("settings.archives.count", {
        count: series.length,
        plural: series.length !== 1 ? "s" : "",
      })}
      bodyClassName="p-0"
    >
      <ArchivesTable series={series} />
    </SettingsCard>
  );
}

"use client";

import { useEffect, useState } from "react";
import { Card, Icon } from "@/app/components/ui";
import type { ArchivedSeriesItemDto } from "@/lib/api";
import { ArchivesTable } from "../archives/ArchivesTable";

export function ArchivesTab() {
  const [series, setSeries] = useState<ArchivedSeriesItemDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/series/archived")
      .then((r) => r.json())
      .then((data) => setSeries(Array.isArray(data) ? data : []))
      .catch(() => setError("Erreur de chargement"));
  }, []);

  if (error) {
    return <p className="text-destructive text-sm">{error}</p>;
  }

  if (!series) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground text-sm">
        <Icon name="spinner" size="sm" className="animate-spin" />
        Chargement…
      </div>
    );
  }

  if (series.length === 0) {
    return (
      <Card>
        <div className="py-12 text-center">
          <Icon name="books" size="lg" className="text-muted-foreground mx-auto mb-3" />
          <p className="text-muted-foreground">Aucune série archivée.</p>
          <p className="text-sm text-muted-foreground mt-1">
            Les séries supprimées du disque apparaîtront ici et leurs états de lecture seront préservés.
          </p>
        </div>
      </Card>
    );
  }

  return (
    <div>
      <p className="text-sm text-muted-foreground mb-4">
        {series.length} série{series.length !== 1 ? "s" : ""} archivée{series.length !== 1 ? "s" : ""}
      </p>
      <Card>
        <ArchivesTable series={series} />
      </Card>
    </div>
  );
}

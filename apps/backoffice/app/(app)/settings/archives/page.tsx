import { listArchivedSeries } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { Icon } from "@/app/components/ui/Icon";
import { Card } from "@/app/components/ui/Card";
import Link from "next/link";
import { ArchivesTable } from "./ArchivesTable";

export const dynamic = "force-dynamic";

export default async function ArchivesPage() {
  const { t } = await getServerTranslations();
  const archivedSeries = await listArchivedSeries().catch(() => []);

  return (
    <div>
      <div className="mb-6">
        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-2">
          <Link href="/settings" className="hover:text-foreground transition-colors">
            {t("settings.title")}
          </Link>
          <Icon name="chevronRight" size="sm" />
          <span>{t("settings.archives")}</span>
        </div>
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="books" size="xl" className="text-amber-500" />
          {t("settings.archives")}
        </h1>
        <p className="text-muted-foreground mt-1">
          {archivedSeries.length} série{archivedSeries.length !== 1 ? "s" : ""} archivée{archivedSeries.length !== 1 ? "s" : ""}
        </p>
      </div>

      {archivedSeries.length === 0 ? (
        <Card>
          <div className="py-12 text-center">
            <Icon name="books" size="lg" className="text-muted-foreground mx-auto mb-3" />
            <p className="text-muted-foreground">Aucune série archivée.</p>
            <p className="text-sm text-muted-foreground mt-1">
              Les séries supprimées du disque apparaîtront ici et leurs états de lecture seront préservés.
            </p>
          </div>
        </Card>
      ) : (
        <Card>
          <ArchivesTable series={archivedSeries} />
        </Card>
      )}
    </div>
  );
}

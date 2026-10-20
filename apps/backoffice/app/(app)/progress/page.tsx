import { getReadingOverview } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { Icon } from "@/app/components/ui";
import { ReadingOverview } from "@/app/components/ReadingOverview";

export const dynamic = "force-dynamic";

export default async function ProgressPage() {
  const { t } = await getServerTranslations();
  const overview = await getReadingOverview();

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="chart" size="xl" className="text-success" />
          {t("nav.progress")}
        </h1>
      </div>

      <ReadingOverview initialData={overview} />
    </>
  );
}

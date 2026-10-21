import {
  fetchDownloadsEnabled,
  fetchLibraries,
  fetchTelegramAuthorized,
  hasEnabledDdlProvider,
  IndexJobDto,
  LibraryDto,
  listJobs,
} from "@/lib/api";
import { JobsList } from "@/app/components/JobsList";
import { getServerTranslations } from "@/lib/i18n/server";

import { JobActions } from "./components/JobActions";

export const dynamic = "force-dynamic";

export default async function JobsPage({
  searchParams,
}: {
  searchParams: Promise<{ highlight?: string; library?: string | string[]; error?: string }>;
}) {
  const { highlight, library, error: errorMsg } = await searchParams;
  const { t } = await getServerTranslations();
  const [jobs, libraries, prowlarrConfigured, telegramAuthorized, ddlConfigured] = await Promise.all([
    listJobs().catch(() => [] as IndexJobDto[]),
    fetchLibraries().catch(() => [] as LibraryDto[]),
    fetchDownloadsEnabled(),
    fetchTelegramAuthorized(),
    hasEnabledDdlProvider(),
  ]);

  const initialLibraries =
    library === undefined
      ? null
      : (Array.isArray(library) ? library : [library]).filter((id) => id.length > 0);

  const libraryMap = new Map(libraries.map((item) => [item.id, item.name]));
  const readingStatusLibraries = libraries.filter((item) => item.reading_status_provider);

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          {t("jobs.title")}
        </h1>
      </div>

      {errorMsg && (
        <div className="mb-4 p-3 rounded-lg bg-destructive/10 border border-destructive/20 text-destructive text-sm">
          {errorMsg}
        </div>
      )}

      <JobActions
        libraries={libraries.map((item) => ({ id: item.id, name: item.name }))}
        initialSelected={initialLibraries}
        readingStatus={readingStatusLibraries.length > 0}
        prowlarr={prowlarrConfigured}
        ddl={ddlConfigured}
        telegram={telegramAuthorized}
      />

      <JobsList initialJobs={jobs} libraries={libraryMap} highlightJobId={highlight} />
    </>
  );
}

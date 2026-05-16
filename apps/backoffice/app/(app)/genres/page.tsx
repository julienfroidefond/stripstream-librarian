import { apiFetch, fetchLibraries } from "@/lib/api";
import type { SeriesDto, LibraryDto } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { GenresClient, type GenreDto } from "./GenresClient";

export const dynamic = "force-dynamic";

export default async function GenresPage() {
  const { t } = await getServerTranslations();

  const [genres, untagged, libraries, seriesPage] = await Promise.all([
    apiFetch<GenreDto[]>("/genres").catch(() => [] as GenreDto[]),
    apiFetch<SeriesDto[]>("/genres/untagged-series").catch(() => [] as SeriesDto[]),
    fetchLibraries().catch(() => [] as LibraryDto[]),
    apiFetch<{ total: number }>("/series?limit=1").catch(() => ({ total: 0 })),
  ]);

  return (
    <div className="space-y-6">
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-pink-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 7h.01M7 3h5a1.99 1.99 0 011.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
          </svg>
          {t("genres.title")}
        </h1>
      </div>
      <GenresClient initialGenres={genres} initialUntagged={untagged} libraries={libraries} initialTotalSeries={seriesPage.total} />
    </div>
  );
}

/**
 * Chargement de la page détail d'une série.
 *
 * Server-only : aucun React, aucun JSX. Le module possède tout le fan-out de
 * fetch de la page afin que celle-ci ne fasse plus que de la composition et
 * que la barre d'actions reçoive un unique view model déjà résolu.
 *
 * Politique de dégradation — deux niveaux, jamais un `.catch` local :
 * - **fatal** (indisponible ⇒ `not-found`) : la série elle-même et sa
 *   bibliothèque. Sans eux il n'y a rien à afficher.
 * - **non fatal** (valeur vide/défaut + nom de la section dans
 *   `model.degraded`) : tout ce qui se rend autour d'un morceau manquant —
 *   la page de livres courante (grille vide, comme avant), métadonnées, liens
 *   de métadonnées, lien AniList, fournisseurs masqués, formats de renommage,
 *   séries similaires, listes de lecture, notes, activité des lecteurs, tomes
 *   possédés et rapport de tomes manquants.
 *
 * La readiness d'acquisition n'est pas dégradée ici : `loadAcquisitionReadiness`
 * porte déjà son propre repli et ne lève pas.
 */

import {
  apiFetch,
  fetchBooks,
  fetchLibraries,
  fetchRelatedSeries,
  fetchSeriesById,
  fetchSeriesMetadata,
  fetchSeriesOwnedVolumes,
  fetchSeriesRatings,
  fetchSeriesReadingLists,
  fetchSeriesUserActivity,
  getBookCoverUrl,
  getMetadataLink,
  getMissingBooks,
  getReadingStatusLink,
} from "@/lib/api";
import type {
  AnilistSeriesLinkDto,
  BookDto,
  ExternalMetadataLinkDto,
  LibraryDto,
  MissingBooksDto,
  ReadingListDto,
  RelatedSeriesDto,
  SeriesDto,
  SeriesMetadataDto,
  SeriesRatingsDto,
  SeriesUserActivityDto,
} from "@/lib/api";

import type { AcquisitionReadiness } from "@/lib/acquisitionReadiness";
import { loadAcquisitionReadiness } from "@/lib/acquisitionReadiness.server";
import { getActiveUser } from "@/lib/activeUser";
import { ratedActivity, readingActivity, type RatedSeriesUserActivity } from "@/lib/userActivity";
import type { ViewMode } from "@/lib/viewMode";

/** Section whose failure degrades the page instead of failing it. */
export type SeriesDetailSection =
  | "books"
  | "metadata"
  | "metadataLinks"
  | "readingStatusLink"
  | "metadataProviders"
  | "renameFormats"
  | "relatedSeries"
  | "readingLists"
  | "ratings"
  | "userActivity"
  | "ownedVolumes"
  | "missingBooks";

/** A book plus the cover URL the grid renders. */
export type SeriesBook = BookDto & { coverUrl: string };

export type SeriesRenameFormats = {
  regular: string | null;
  hs: string | null;
  integral: string | null;
  oneshot: string | null;
};

/** View model of the actions toolbar, derived from the page model. */
export type SeriesActionsModel = {
  libraryId: string;
  seriesId: string;
  seriesName: string;
  bookCount: number;
  booksReadCount: number;
  hasActiveUser: boolean;
  isShelved: boolean;
  metadata: SeriesMetadataDto | null;
  /** Book-level edit defaults, already falling back to the first book. */
  editBookAuthor: string | null;
  editBookLanguage: string | null;
  metadataLinks: ExternalMetadataLinkDto[];
  existingLink: ExternalMetadataLinkDto | null;
  missingData: MissingBooksDto | null;
  hiddenProviders: string[];
  readingStatusProvider: string | null;
  readingStatusLink: AnilistSeriesLinkDto | null;
  readiness: AcquisitionReadiness;
  ownedVolumes: number[] | null;
  renameFormats: SeriesRenameFormats;
};

/** Everything the series detail page renders, already resolved. */
export type SeriesDetailModel = {
  seriesId: string;
  seriesName: string;
  series: SeriesDto;
  library: LibraryDto;
  page: number;
  limit: number;
  viewMode: ViewMode;
  books: SeriesBook[];
  booksTotal: number;
  totalPages: number;
  booksReadCount: number;
  hasActiveUser: boolean;
  activeUserId: string;
  metadata: SeriesMetadataDto | null;
  metadataLinks: ExternalMetadataLinkDto[];
  existingLink: ExternalMetadataLinkDto | null;
  missingData: MissingBooksDto | null;
  readingStatusLink: AnilistSeriesLinkDto | null;
  hiddenProviders: string[];
  relatedSeries: RelatedSeriesDto[];
  readingLists: ReadingListDto[];
  ratings: SeriesRatingsDto | null;
  ratedEntries: RatedSeriesUserActivity[];
  readingEntries: SeriesUserActivityDto[];
  actions: SeriesActionsModel;
  /** Non-fatal sections that failed and fell back to an empty value. */
  degraded: SeriesDetailSection[];
};

export type SeriesDetailResult =
  | { status: "ok"; model: SeriesDetailModel }
  | { status: "not-found" };

export type SeriesDetailOptions = {
  page: number;
  limit: number;
  viewMode: ViewMode;
};

type Outcome<T> = { ok: true; value: T } | { ok: false };

/** Resolves to a tagged outcome instead of rejecting. */
function optional<T>(promise: Promise<T>): Promise<Outcome<T>> {
  return promise.then(
    (value) => ({ ok: true as const, value }),
    () => ({ ok: false as const }),
  );
}

function stringSetting(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

export async function loadSeriesDetail(
  seriesId: string,
  { page, limit, viewMode }: SeriesDetailOptions,
): Promise<SeriesDetailResult> {
  const [seriesOutcome, activeUser] = await Promise.all([
    optional(fetchSeriesById(seriesId)),
    getActiveUser(),
  ]);  if (!seriesOutcome.ok) return { status: "not-found" };
  const series = seriesOutcome.value;

  // La bibliothèque porte le breadcrumb et le fournisseur de statut de
  // lecture : sans elle la page n'a rien de cohérent à rendre. Son fetch n'est
  // pas dégradé (comme avant) : une panne de `/libraries` remonte en erreur.
  const library = (await fetchLibraries()).find((item) => item.id === series.library_id);
  if (!library) return { status: "not-found" };

  const [
    booksOutcome,
    metadataOutcome,
    metadataLinksOutcome,
    readingStatusLinkOutcome,
    metadataProvidersOutcome,
    renameFormatOutcome,
    renameFormatHsOutcome,
    renameFormatIntOutcome,
    renameFormatOneshotOutcome,
    relatedSeriesOutcome,
    readingListsOutcome,
    ratingsOutcome,
    userActivityOutcome,
    ownedVolumesOutcome,
    readiness,
  ] = await Promise.all([
    optional(fetchBooks(library.id, seriesId, page, limit)),
    optional(fetchSeriesMetadata(seriesId)),
    optional(getMetadataLink(seriesId)),
    optional(getReadingStatusLink(seriesId)),
    optional(
      apiFetch<{ comicvine?: { api_key?: string } }>("/settings/metadata_providers", {
        next: { revalidate: 3600 },
      }),
    ),
    optional(apiFetch<string>("/settings/rename_format", { next: { revalidate: 3600 } })),
    optional(apiFetch<string>("/settings/rename_format_hs", { next: { revalidate: 3600 } })),
    optional(apiFetch<string>("/settings/rename_format_int", { next: { revalidate: 3600 } })),
    optional(apiFetch<string>("/settings/rename_format_oneshot", { next: { revalidate: 3600 } })),
    optional(fetchRelatedSeries(seriesId, 12)),
    optional(fetchSeriesReadingLists(seriesId)),
    optional(fetchSeriesRatings(seriesId)),
    // En mode admin (aucun lecteur sélectionné), l'activité par série remplace
    // la vue mono-lecteur : inutile de la requêter.
    activeUser.hasActiveUser
      ? Promise.resolve({ ok: true as const, value: [] as SeriesUserActivityDto[] })
      : optional(fetchSeriesUserActivity(seriesId)),
    optional(fetchSeriesOwnedVolumes(seriesId)),
    loadAcquisitionReadiness(),
  ]);

  // La page de livres se dégrade (grille vide + section signalée), exactement
  // comme avant : une panne de `/books` ne doit pas transformer une série
  // existante en 404.
  const booksPage = booksOutcome.ok
    ? booksOutcome.value
    : { items: [] as BookDto[], total: 0, page: 1, limit };

  const degraded: SeriesDetailSection[] = [];
  if (!booksOutcome.ok) degraded.push("books");
  if (!metadataOutcome.ok) degraded.push("metadata");
  if (!metadataLinksOutcome.ok) degraded.push("metadataLinks");
  if (!readingStatusLinkOutcome.ok) degraded.push("readingStatusLink");
  if (!metadataProvidersOutcome.ok) degraded.push("metadataProviders");
  if (
    !renameFormatOutcome.ok ||
    !renameFormatHsOutcome.ok ||
    !renameFormatIntOutcome.ok ||
    !renameFormatOneshotOutcome.ok
  ) {
    degraded.push("renameFormats");
  }
  if (!relatedSeriesOutcome.ok) degraded.push("relatedSeries");
  if (!readingListsOutcome.ok) degraded.push("readingLists");
  if (!ratingsOutcome.ok) degraded.push("ratings");
  if (!userActivityOutcome.ok) degraded.push("userActivity");
  if (!ownedVolumesOutcome.ok) degraded.push("ownedVolumes");

  const metadata = metadataOutcome.ok ? metadataOutcome.value : null;
  const metadataLinks = metadataLinksOutcome.ok ? metadataLinksOutcome.value : [];
  const readingStatusLink = readingStatusLinkOutcome.ok ? readingStatusLinkOutcome.value : null;
  const metadataProviders = metadataProvidersOutcome.ok ? metadataProvidersOutcome.value : null;
  const relatedSeries = relatedSeriesOutcome.ok ? relatedSeriesOutcome.value : [];
  const readingLists = readingListsOutcome.ok ? readingListsOutcome.value : [];
  const ratings = ratingsOutcome.ok ? ratingsOutcome.value : null;
  const userActivity = userActivityOutcome.ok ? userActivityOutcome.value : [];
  const ownedVolumes = ownedVolumesOutcome.ok ? ownedVolumesOutcome.value : null;

  const seriesName = metadata?.series_name ?? "";

  const existingLink =
    metadataLinks.find((link) => link.status === "approved" && link.is_primary) ??
    metadataLinks.find((link) => link.status === "approved") ??
    metadataLinks[0] ??
    null;

  let missingData: MissingBooksDto | null = null;
  if (existingLink?.status === "approved") {
    const missingOutcome = await optional(getMissingBooks(existingLink.id));
    if (missingOutcome.ok) {
      missingData = missingOutcome.value;
    } else {
      degraded.push("missingBooks");
    }
  }

  const hiddenProviders: string[] = [];
  if (!metadataProviders?.comicvine?.api_key) hiddenProviders.push("comicvine");

  const renameFormats: SeriesRenameFormats = {
    regular: renameFormatOutcome.ok ? stringSetting(renameFormatOutcome.value) : null,
    hs: renameFormatHsOutcome.ok ? stringSetting(renameFormatHsOutcome.value) : null,
    integral: renameFormatIntOutcome.ok ? stringSetting(renameFormatIntOutcome.value) : null,
    oneshot: renameFormatOneshotOutcome.ok ? stringSetting(renameFormatOneshotOutcome.value) : null,
  };

  const books: SeriesBook[] = booksPage.items.map((book) => ({
    ...book,
    coverUrl: getBookCoverUrl(book.id, book.updated_at),
  }));
  const booksReadCount = booksPage.items.filter((book) => book.reading_status === "read").length;

  // Valeurs par défaut du formulaire d'édition : la métadonnée de série prime,
  // le premier livre sert de repli.
  const editBookAuthor = metadata?.book_author ?? booksPage.items[0]?.author ?? null;
  const editBookLanguage = metadata?.book_language ?? booksPage.items[0]?.language ?? null;

  const actions: SeriesActionsModel = {
    libraryId: library.id,
    seriesId,
    seriesName,
    bookCount: booksPage.total,
    booksReadCount,
    hasActiveUser: activeUser.hasActiveUser,
    isShelved: Boolean(series.shelved_at),
    metadata,
    editBookAuthor,
    editBookLanguage,
    metadataLinks,
    existingLink,
    missingData,
    hiddenProviders,
    readingStatusProvider: library.reading_status_provider ?? null,
    readingStatusLink,
    readiness,
    ownedVolumes,
    renameFormats,
  };

  return {
    status: "ok",
    model: {
      seriesId,
      seriesName,
      series,
      library,
      page,
      limit,
      viewMode,
      books,
      booksTotal: booksPage.total,
      totalPages: Math.ceil(booksPage.total / limit),
      booksReadCount,
      hasActiveUser: activeUser.hasActiveUser,
      activeUserId: activeUser.userId ?? "admin",
      metadata,
      metadataLinks,
      existingLink,
      missingData,
      readingStatusLink,
      hiddenProviders,
      relatedSeries,
      readingLists,
      ratings,
      ratedEntries: ratedActivity(userActivity),
      readingEntries: readingActivity(userActivity),
      actions,
      degraded,
    },
  };
}

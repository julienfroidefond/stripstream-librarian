/**
 * Genres : liste, navigation par genres (requête serveur unique avec facettes).
 */

import { apiFetch } from "./client";
import type { SeriesDto } from "./series";
import { genreBrowsePath } from "./browser/genres";

export type GenreDto = {
  name: string;
  series_count: number;
};

/** Série représentative d'un genre (aperçu de couverture). */
export type GenreCover = {
  series_id: string;
  name: string;
  first_book_id: string | null;
  first_book_updated_at: string | null;
  cover_url: string | null;
};

export type GenreFacet = GenreDto & {
  cover: GenreCover | null;
};

/** Filtres de `GET /genres/browse`. Les facettes ne dépendent que de `libraryIds`. */
export type GenreBrowseQuery = {
  /** Genres requis (ET). */
  genres?: readonly string[];
  /** Genres exclus. */
  exclude?: readonly string[];
  /** Périmètre de bibliothèques (vide = toutes). */
  libraryIds?: readonly string[];
  /** Uniquement les séries sans genre. */
  untagged?: boolean;
  q?: string;
  page?: number;
  limit?: number;
};

export type GenreBrowsePage = {
  items: SeriesDto[];
  total: number;
  page: number;
  limit: number;
  facets: {
    genres: GenreFacet[];
    untagged: number;
    total: number;
  };
};

export async function fetchGenreBrowse(query: GenreBrowseQuery = {}): Promise<GenreBrowsePage> {
  return apiFetch<GenreBrowsePage>(genreBrowsePath("/genres/browse", query));
}

import { revalidatePath, revalidateTag } from "next/cache";

import { cacheTags } from "./tags";

/**
 * Domain writes that make cached reads stale. Each route handler or server
 * action states *what happened*; this module alone decides which tags and
 * paths expire.
 */
export type DomainWrite =
  | { type: "series.updated"; seriesId: string }
  | { type: "series.deleted"; seriesId: string }
  | { type: "series.merged"; targetId: string; sourceId?: string }
  | { type: "series.created" }
  | { type: "series.booksRenamed"; seriesId: string }
  | { type: "series.orphansDeleted" }
  | { type: "book.updated"; bookId: string; seriesId?: string | null }
  | { type: "book.deleted"; bookId: string; seriesId?: string | null }
  | { type: "readingProgress.changed"; bookId?: string; seriesId?: string | null }
  | { type: "genres.changed"; seriesIds?: string[] }
  | { type: "library.created" }
  | { type: "library.deleted"; libraryId: string }
  | { type: "library.updated"; libraryId: string }
  | { type: "readingLists.changed" }
  | { type: "metadata.changed"; seriesId?: string; linkId?: string }
  | { type: "jobs.started" }
  | { type: "jobs.finished" }
  | { type: "activeUser.changed" }
  | { type: "tokens.changed" }
  | { type: "users.changed" }
  | { type: "catalog.refreshRequested"; scope: "books" | "series" }
  | { type: "catalog.refreshRequested"; scope: "series-detail"; seriesId: string };

/** A path to revalidate; `layout` also expires every page below it. */
export type CachePath = { path: string; type?: "layout" | "page" };

export interface InvalidationPlan {
  tags: string[];
  paths: CachePath[];
}

const page = (path: string): CachePath => ({ path });

/** Paths of the pages rendering a series (list + optional detail). */
function seriesPaths(...ids: Array<string | null | undefined>): CachePath[] {
  return [page("/series"), ...ids.filter((id): id is string => !!id).map((id) => page(`/series/${id}`))];
}

function seriesTags(...ids: Array<string | null | undefined>): string[] {
  return [cacheTags.series(), ...ids.filter((id): id is string => !!id).map(cacheTags.seriesById)];
}

/** Pure mapping from a domain write to the cache entries it makes stale. */
export function invalidationPlan(write: DomainWrite): InvalidationPlan {
  switch (write.type) {
    case "series.updated":
      return { tags: seriesTags(write.seriesId), paths: seriesPaths(write.seriesId) };
    case "series.deleted":
      return {
        tags: [
          ...seriesTags(write.seriesId),
          cacheTags.books(),
          cacheTags.metadataGaps(),
          cacheTags.readingLists(),
          cacheTags.stats(),
        ],
        paths: [...seriesPaths(write.seriesId), page("/books"), page("/reading-lists")],
      };
    case "series.merged":
      return {
        tags: [
          ...seriesTags(write.targetId, write.sourceId),
          cacheTags.books(),
          cacheTags.metadata(),
          cacheTags.metadataGaps(),
          cacheTags.readingLists(),
        ],
        paths: [...seriesPaths(write.targetId, write.sourceId), page("/books"), page("/reading-lists")],
      };
    case "series.created":
      return {
        tags: [cacheTags.series(), cacheTags.metadataGaps()],
        paths: [page("/series"), page("/libraries")],
      };
    case "series.booksRenamed":
      return {
        tags: [...seriesTags(write.seriesId), cacheTags.books()],
        paths: [...seriesPaths(write.seriesId), page("/books")],
      };
    case "series.orphansDeleted":
      return {
        tags: [cacheTags.series(), cacheTags.metadataGaps()],
        paths: [page("/series"), page("/settings")],
      };
    case "book.updated":
      return {
        tags: [cacheTags.books(), ...seriesTags(write.seriesId)],
        paths: [page("/books"), page(`/books/${write.bookId}`), ...seriesPaths(write.seriesId)],
      };
    case "book.deleted":
      return {
        tags: [cacheTags.books(), ...seriesTags(write.seriesId), cacheTags.stats()],
        paths: [page("/books"), ...seriesPaths(write.seriesId)],
      };
    case "readingProgress.changed":
      return {
        // The dashboard caches its stats 30 s; progress must show up at once
        // ("in progress" / "recent" lists).
        tags: [cacheTags.stats(), cacheTags.books(), ...seriesTags(write.seriesId)],
        paths: [
          page("/"),
          ...(write.bookId ? [page(`/books/${write.bookId}`)] : []),
          ...seriesPaths(write.seriesId),
        ],
      };
    case "genres.changed":
      return {
        tags: [...seriesTags(...(write.seriesIds ?? [])), cacheTags.metadataGaps()],
        paths: [page("/genres"), page("/series")],
      };
    case "library.created":
      return { tags: [], paths: [page("/libraries")] };
    case "library.deleted":
      return {
        tags: [cacheTags.series(), cacheTags.books(), cacheTags.stats()],
        paths: [page("/libraries"), page(`/libraries/${write.libraryId}`), page("/series"), page("/books")],
      };
    case "library.updated":
      return { tags: [], paths: [page("/libraries"), page(`/libraries/${write.libraryId}`)] };
    case "readingLists.changed":
      // `revalidateTag` alone can leave the already-rendered grouped Series
      // page serving its stale fetch payload on the next request in dev: the
      // paths are expired too.
      return {
        tags: [cacheTags.readingLists()],
        paths: [page("/series"), page("/reading-lists")],
      };
    case "metadata.changed":
      return {
        tags: [
          cacheTags.metadata(),
          ...seriesTags(write.seriesId),
          ...(write.linkId ? [cacheTags.metadataLink(write.linkId)] : []),
        ],
        paths: [],
      };
    case "jobs.started":
      return { tags: [], paths: [page("/jobs")] };
    case "jobs.finished":
      return { tags: [cacheTags.books(), cacheTags.series(), cacheTags.stats()], paths: [] };
    case "tokens.changed":
    case "users.changed":
      // Tokens and readers are only listed on the settings page (uncached reads).
      return { tags: [], paths: [page("/settings")] };
    case "activeUser.changed":
      return { tags: [], paths: [{ path: "/", type: "layout" }] };
    case "catalog.refreshRequested":
      switch (write.scope) {
        case "books":
          return { tags: [cacheTags.books()], paths: [] };
        case "series":
          return { tags: [cacheTags.series()], paths: [] };
        case "series-detail":
          return { tags: [cacheTags.seriesById(write.seriesId)], paths: [] };
      }
  }
}

/**
 * Expires every cached read made stale by `write`.
 *
 * Tags expire with `{ expire: 0 }`: the next request blocks on fresh data
 * instead of serving stale content (works in route handlers and server
 * actions alike).
 */
export function invalidateAfter(write: DomainWrite): void {
  const { tags, paths } = invalidationPlan(write);
  for (const tag of new Set(tags)) {
    revalidateTag(tag, { expire: 0 });
  }
  for (const { path, type } of paths) {
    if (type) revalidatePath(path, type);
    else revalidatePath(path);
  }
}

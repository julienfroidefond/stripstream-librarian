/**
 * Cache tags of the Next data cache.
 *
 * Every reader (`next: { tags: [...] }`) and every invalidation
 * (`invalidateAfter`) builds its tags here, so a reader and the writes that
 * must expire it cannot drift apart on a string literal.
 *
 * Pure module: safe to import from `lib/api/*` readers.
 */
export const cacheTags = {
  /** Book lists and book-level reads. */
  books: () => "books",
  /** Any series read (lists, ratings, aggregates). */
  series: () => "series",
  /** Reads scoped to one series (detail, metadata, reading-list memberships). */
  seriesById: (seriesId: string) => `series:${seriesId}`,
  /** Series missing metadata. */
  metadataGaps: () => "metadata-gaps",
  /** External metadata (links, candidates, approvals). */
  metadata: () => "metadata",
  /** One external metadata link. */
  metadataLink: (linkId: string) => `link:${linkId}`,
  /** Reading lists and their memberships. */
  readingLists: () => "reading-lists",
  /** Dashboard statistics. */
  stats: () => "stats",
} as const;

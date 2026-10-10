-- Performance indexes for hot read paths identified in the API perf audit.

-- /books default sort is a natural (tome-aware) ordering built from expressions
-- on `title` and `volume`. Without a matching index every library listing does a
-- full scan + top-N sort of `books`. The expressions below MUST stay in sync
-- with `order_clause` (default branch) in apps/api/src/books/mod.rs.
CREATE INDEX IF NOT EXISTS idx_books_natural_sort
    ON books (
        library_id,
        volume,
        regexp_replace(lower(title), '[0-9].*$', ''),
        (COALESCE((regexp_match(lower(title), '\d+'))[1]::int, 0)),
        title
    );

-- /stats jobs-over-time aggregates filter `index_jobs` by terminal status and a
-- `finished_at` range; a partial index keeps that scan cheap as jobs accumulate.
CREATE INDEX IF NOT EXISTS idx_index_jobs_finished
    ON index_jobs (finished_at)
    WHERE status IN ('success', 'failed');

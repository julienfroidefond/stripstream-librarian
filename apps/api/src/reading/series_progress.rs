//! Shared SQL for per-reader series progress aggregation.
//!
//! The fragment assumes `book_reading_progress AS brp` joined with
//! `books AS b`; callers add their own grouping columns and `GROUP BY`.

/// Read/reading counters plus the latest activity timestamp for the group.
pub(crate) const SERIES_PROGRESS_AGGREGATES: &str = r#"
    COUNT(*) FILTER (WHERE brp.status = 'read') AS books_read,
    COUNT(*) FILTER (WHERE brp.status = 'reading') AS books_reading,
    MAX(brp.last_read_at) AS last_read_at
"#;

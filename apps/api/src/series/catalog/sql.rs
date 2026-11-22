use sqlx::postgres::PgArguments;
use sqlx::query::Query;
use sqlx::Postgres;
use uuid::Uuid;

use super::{Gap, MetadataLink, RatingFilter, SeriesRequest, SeriesScope, SeriesSort};
use crate::series::helpers;

/// The reading status expression shared by the filter and the projection.
const READING_STATUS_EXPR: &str = r#"CASE
              WHEN sc.book_count = 0 THEN 'unread'
              WHEN sc.books_read_count = sc.book_count THEN 'read'
              WHEN sc.books_read_count = 0 THEN 'unread'
              ELSE 'reading'
           END"#;

/// Used in SELECT and ORDER BY — the index on (series_id, status) makes this
/// fast per row.
const COMMUNITY_SCORE_LATERAL: &str = r#"LEFT JOIN LATERAL (
        SELECT (AVG(provider_rating / COALESCE(NULLIF(provider_rating_scale, 0), 10.0) * 5.0))::real AS community_score
        FROM external_metadata_links
        WHERE series_id = sc.series_id
          AND status = 'approved'
          AND provider_rating IS NOT NULL
          AND provider_rating > 0
    ) cs ON TRUE"#;

/// One bound value. `build` assigns placeholders and parameters in the same
/// order, so the query is assembled once and bound without re-counting.
pub(super) enum Param {
    Uuid(Uuid),
    OptUuid(Option<Uuid>),
    Text(String),
    Texts(Vec<String>),
    Int(i64),
}

impl Param {
    pub(super) fn bind<'q>(
        self,
        query: Query<'q, Postgres, PgArguments>,
    ) -> Query<'q, Postgres, PgArguments> {
        match self {
            Self::Uuid(value) => query.bind(value),
            Self::OptUuid(value) => query.bind(value),
            Self::Text(value) => query.bind(value),
            Self::Texts(value) => query.bind(value),
            Self::Int(value) => query.bind(value),
        }
    }
}

#[derive(Default)]
struct Binder {
    params: Vec<Param>,
}

impl Binder {
    fn push(&mut self, param: Param) -> String {
        self.params.push(param);
        format!("${}", self.params.len())
    }
}

/// What a scope contributes to the query: its staging CTEs, its
/// `series_counts` body and its order.
struct ScopeSql {
    /// CTEs emitted before `series_counts`, already comma-terminated.
    preamble: String,
    /// The body of `series_counts AS (...)`.
    series_counts: String,
    /// The scope's order key selected from `sc` in `filtered`, with its leading
    /// comma; empty when the key is already projected.
    order_key: String,
    /// `ORDER BY` body, evaluated against `filtered`'s columns.
    order: String,
}

/// The cross-library list and the detail: both read the whole `series` table.
fn all_scope(where_clause: &str, user: &str, sort: SeriesSort) -> ScopeSql {
    let tome_counts = helpers::series_tome_counts("brp");
    ScopeSql {
        preamble: String::new(),
        series_counts: format!(
            r#"
            SELECT
                s.id as series_id,
                s.name,
                s.library_id,
                {tome_counts},
                COALESCE(MAX(b.created_at), s.created_at) as latest_created_at
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND {user}::uuid IS NOT NULL AND brp.user_id = {user}
            {where_clause}
            GROUP BY s.id, s.name, s.library_id, s.created_at"#
        ),
        order_key: ", sc.latest_created_at".to_string(),
        order: sort_order(sort),
    }
}

/// The reader's favourites: the scope owns the ordering, by favourite time.
fn favorites_scope(user: &str) -> ScopeSql {
    let tome_counts = helpers::series_tome_counts("brp");
    ScopeSql {
        preamble: String::new(),
        series_counts: format!(
            r#"
            SELECT
                s.id AS series_id,
                s.name,
                s.library_id,
                {tome_counts},
                f.created_at AS favorited_at
            FROM series_user_favorites f
            JOIN series s ON s.id = f.series_id
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = {user}
            WHERE f.user_id = {user}
            GROUP BY f.created_at, s.id, s.name, s.library_id"#
        ),
        order_key: ", sc.favorited_at".to_string(),
        order: "favorited_at DESC".to_string(),
    }
}

/// The reader's ongoing series, built from their progress so the scan starts
/// on a small set instead of every book.
fn ongoing_scope(user: &str) -> ScopeSql {
    let tome_counts = helpers::series_tome_counts("up");
    ScopeSql {
        preamble: format!(
            r#"
    user_progress AS (
        SELECT book_id, status, last_read_at
        FROM book_reading_progress
        WHERE user_id = {user}
    ),
    active_series AS (
        SELECT DISTINCT b.series_id
        FROM user_progress up
        JOIN books b ON b.id = up.book_id
        WHERE up.status IN ('read', 'reading')
    ),"#
        ),
        series_counts: format!(
            r#"
            SELECT * FROM (
                SELECT
                    s.id AS series_id,
                    s.name,
                    s.library_id,
                    {tome_counts}
                FROM active_series a_s
                JOIN series s ON s.id = a_s.series_id
                JOIN books b ON b.series_id = s.id
                LEFT JOIN user_progress up ON up.book_id = b.id
                GROUP BY s.id, s.name, s.library_id
            ) stats
            WHERE stats.books_started_count > 0
              AND stats.books_read_count < stats.book_count"#
        ),
        order_key: String::new(),
        order: "last_read_at DESC NULLS LAST".to_string(),
    }
}

/// The order of a `title`/`latest`/`release_date`/`community_score` page.
fn sort_order(sort: SeriesSort) -> String {
    const TITLE: &str = "lower(name) ASC";
    match sort {
        SeriesSort::Title => TITLE.to_string(),
        // For series without books, latest_created_at falls back to
        // s.created_at (see the series_counts CTE), so the value is never NULL.
        SeriesSort::Latest => "latest_created_at DESC".to_string(),
        SeriesSort::ReleaseDate => format!("start_year DESC NULLS LAST, {TITLE}"),
        SeriesSort::CommunityScore => {
            format!("community_score DESC NULLS LAST, {TITLE}")
        }
        // Owned by the ongoing scope, which never calls this.
        SeriesSort::Activity => TITLE.to_string(),
    }
}

/// Build the catalogue SQL and its bound parameters.
pub(super) fn build(request: &SeriesRequest) -> (String, Vec<Param>) {
    let mut binder = Binder::default();
    let filters = &request.filters;

    // The scope comes first so the missing-count CTE can reuse its predicate.
    let (scope_where, scope_predicate) = match &request.scope {
        SeriesScope::All { library_id } => match library_id {
            Some(library_id) => {
                let param = binder.push(Param::Uuid(*library_id));
                let predicate = format!("s.library_id = {param}");
                (format!("WHERE {predicate}"), Some(predicate))
            }
            None => (String::new(), None),
        },
        SeriesScope::One { series_id } => {
            let param = binder.push(Param::Uuid(*series_id));
            let predicate = format!("s.id = {param}");
            (format!("WHERE {predicate}"), Some(predicate))
        }
        SeriesScope::Favorites | SeriesScope::Ongoing => (String::new(), None),
    };

    // Always bound (NULL when no reader is selected) so the CTE can test it
    // without a second query shape.
    let user_param = binder.push(Param::OptUuid(request.user_id));

    let q_cond = filters.q.as_ref().map_or_else(String::new, |q| {
        let param = binder.push(Param::Text(format!("%{q}%")));
        format!("AND norm_text(s.name) LIKE norm_text({param})")
    });

    let rs_cond = if filters.reading_status.is_empty() {
        String::new()
    } else {
        let statuses: Vec<String> = filters
            .reading_status
            .iter()
            .map(|status| status.as_str().to_string())
            .collect();
        let param = binder.push(Param::Texts(statuses));
        format!("AND {READING_STATUS_EXPR} = ANY({param})")
    };

    let ss_cond = filters
        .series_status
        .as_ref()
        .map_or_else(String::new, |status| {
            let param = binder.push(Param::Text(status.clone()));
            format!("AND LOWER(s.status) = {param}")
        });

    let missing_cond = if filters.has_missing {
        "AND mc.missing_count > 0"
    } else {
        ""
    };

    let metadata_link_cond = match &filters.metadata_link {
        MetadataLink::Any => String::new(),
        MetadataLink::Unlinked => "AND NOT EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved')".to_string(),
        MetadataLink::Linked => "AND EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved')".to_string(),
        MetadataLink::Provider(provider) => {
            let param = binder.push(Param::Text(provider.clone()));
            format!("AND EXISTS (SELECT 1 FROM external_metadata_links eml WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved' AND eml.provider = {param})")
        }
    };

    let gap_cond = match filters.gap {
        None => String::new(),
        Some(Gap::Description) => "AND (s.description IS NULL OR s.description = '')".to_string(),
        Some(Gap::Genre) => "AND COALESCE(cardinality(s.genres), 0) = 0".to_string(),
        Some(Gap::Authors) => "AND COALESCE(cardinality(s.authors), 0) = 0".to_string(),
        Some(Gap::Publishers) => "AND COALESCE(cardinality(s.publishers), 0) = 0".to_string(),
        Some(Gap::Year) => "AND s.start_year IS NULL".to_string(),
        Some(Gap::Cover) => "AND (s.cover_url IS NULL OR s.cover_url = '')".to_string(),
        // No approved provider link carries a usable rating for this series.
        // Must mirror the `COMMUNITY_SCORE_LATERAL` predicate exactly.
        Some(Gap::CommunityScore) => "AND cs.community_score IS NULL".to_string(),
    };

    let author_cond = filters.author.as_ref().map_or_else(String::new, |author| {
        let param = binder.push(Param::Text(author.clone()));
        format!("AND ({param} = ANY(s.authors) OR EXISTS (SELECT 1 FROM books bk WHERE bk.series_id = s.id AND {param} = ANY(COALESCE(NULLIF(bk.authors, '{{}}'), CASE WHEN bk.author IS NOT NULL AND bk.author != '' THEN ARRAY[bk.author] ELSE ARRAY[]::text[] END))))")
    });

    let has_books_cond = if filters.has_books {
        "AND sc.book_count > 0"
    } else {
        ""
    };

    let no_books_cond = if filters.no_books {
        "AND sc.book_count = 0"
    } else {
        ""
    };

    let shelved_cond = match filters.shelved {
        Some(true) => "AND sus.created_at IS NOT NULL",
        Some(false) => "AND sus.created_at IS NULL",
        None => "",
    };

    let genre_cond = filters.genre.as_ref().map_or_else(String::new, |genre| {
        let param = binder.push(Param::Text(genre.clone()));
        format!("AND {param} = ANY(s.genres)")
    });

    let volume_type_cond = filters.volume_type.map_or_else(String::new, |volume_type| {
        format!(
            "AND EXISTS (SELECT 1 FROM books bvt WHERE bvt.series_id = s.id AND bvt.volume_type = '{}')",
            volume_type.as_str()
        )
    });

    let rating_cond = match filters.rating {
        RatingFilter::Any => "",
        RatingFilter::RatedOnly => "AND sur.rating IS NOT NULL",
        RatingFilter::Unrated => "AND sur.rating IS NULL",
    };

    let genre_restriction_cond = format!(
        "AND ({user_param}::uuid IS NULL OR NOT EXISTS (SELECT 1 FROM user_genre_restrictions ugr WHERE ugr.user_id = {user_param} AND ugr.genre = ANY(s.genres)))"
    );

    // A scope that borrows a set (favourites, ongoing) expresses its
    // missing-count predicate as a membership test on that set.
    let missing_predicate = match request.scope {
        SeriesScope::Favorites => Some(format!(
            "s.id IN (SELECT series_id FROM series_user_favorites WHERE user_id = {user_param})"
        )),
        SeriesScope::Ongoing => Some("s.id IN (SELECT series_id FROM active_series)".to_string()),
        SeriesScope::All { .. } | SeriesScope::One { .. } => scope_predicate,
    };

    let scope = match request.scope {
        SeriesScope::All { .. } | SeriesScope::One { .. } => {
            all_scope(&scope_where, &user_param, request.sort)
        }
        SeriesScope::Favorites => favorites_scope(&user_param),
        SeriesScope::Ongoing => ongoing_scope(&user_param),
    };

    // Ongoing never shows a series the reader set aside.
    let scope_cond = match request.scope {
        SeriesScope::Ongoing => "AND sus.created_at IS NULL",
        _ => "",
    };

    let missing_cte =
        crate::coverage::missing_counts_cte_for_predicate(missing_predicate.as_deref());

    let paging = if request.paginated {
        let limit_param = binder.push(Param::Int(request.limit));
        let offset_param = binder.push(Param::Int((request.page - 1) * request.limit));
        format!("LIMIT {limit_param} OFFSET {offset_param}")
    } else {
        String::new()
    };

    let ScopeSql {
        preamble,
        series_counts,
        order_key,
        order: order_clause,
    } = scope;

    let sql = format!(
        r#"
        WITH
        {preamble}
        series_counts AS (
            {series_counts}
        ),
        {missing_cte},
        filtered AS (
            SELECT
                sc.name,
                sc.series_id,
                sc.book_count,
                sc.books_read_count,
                sc.last_read_at,
                sc.library_id,
                s.status as series_status,
                mc.missing_count,
                ml.provider as metadata_provider,
                asl.anilist_id,
                asl.anilist_url,
                s.cover_url, s.start_year, s.genres, s.authors, s.description,
                cs.community_score,
                sur.rating as user_rating,
                sus.created_at as shelved_at{order_key}
            FROM series_counts sc
            LEFT JOIN series s ON s.id = sc.series_id
            LEFT JOIN missing_counts mc ON mc.series_id = sc.series_id
            LEFT JOIN LATERAL (
                SELECT eml.provider FROM external_metadata_links eml
                WHERE eml.series_id = sc.series_id AND eml.library_id = sc.library_id AND eml.status = 'approved'
                ORDER BY eml.is_primary DESC, eml.created_at DESC LIMIT 1
            ) ml ON TRUE
            LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
            {COMMUNITY_SCORE_LATERAL}
            LEFT JOIN series_user_ratings sur ON sur.series_id = sc.series_id AND sur.user_id = {user_param}::uuid
            LEFT JOIN series_user_shelves sus ON sus.series_id = sc.series_id AND sus.user_id = {user_param}::uuid
            WHERE TRUE
              {q_cond}
              {rs_cond}
              {ss_cond}
              {missing_cond}
              {metadata_link_cond}
              {gap_cond}
              {author_cond}
              {has_books_cond}
              {shelved_cond}
              {no_books_cond}
              {genre_cond}
              {volume_type_cond}
              {rating_cond}
              {genre_restriction_cond}
              {scope_cond}
        ),
        total AS (SELECT COUNT(*) AS total_count FROM filtered),
        page AS (
            SELECT * FROM filtered
            ORDER BY {order_clause}
            {paging}
        )
        SELECT p.*, fb.id as first_book_id, fb.updated_at as first_book_updated_at, t.total_count
        FROM total t
        LEFT JOIN page p ON TRUE
        LEFT JOIN LATERAL (
            SELECT b.id, b.updated_at
            FROM books b
            WHERE b.series_id = p.series_id
            ORDER BY
                CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                b.volume NULLS LAST,
                b.title ASC
            LIMIT 1
        ) fb ON TRUE
        ORDER BY {order_clause}
        "#
    );

    (sql, binder.params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::catalog::{ReadingStatus, SeriesFilters, SeriesRequest, VolumeType};

    #[test]
    fn every_placeholder_is_bound() {
        let request = SeriesRequest {
            scope: SeriesScope::All {
                library_id: Some(Uuid::new_v4()),
            },
            filters: SeriesFilters {
                q: Some("dragon".to_string()),
                reading_status: vec![ReadingStatus::Unread, ReadingStatus::Read],
                series_status: Some("ongoing".to_string()),
                has_missing: true,
                metadata_link: MetadataLink::Provider("anilist".to_string()),
                gap: Some(Gap::CommunityScore),
                author: Some("Toriyama".to_string()),
                has_books: true,
                no_books: false,
                shelved: Some(false),
                genre: Some("Fantasy".to_string()),
                volume_type: Some(VolumeType::Oneshot),
                rating: RatingFilter::Unrated,
            },
            sort: SeriesSort::CommunityScore,
            page: 2,
            limit: 10,
            paginated: true,
            user_id: Some(Uuid::new_v4()),
        };

        let (sql, params) = build(&request);

        assert_bound(&sql, params.len());
    }

    #[test]
    fn every_placeholder_is_bound_without_library_or_user() {
        let request = SeriesRequest {
            scope: SeriesScope::All { library_id: None },
            filters: SeriesFilters::default(),
            sort: SeriesSort::Title,
            page: 1,
            limit: 50,
            paginated: true,
            user_id: None,
        };

        let (sql, params) = build(&request);

        assert_bound(&sql, params.len());
    }

    #[test]
    fn the_detail_scope_reads_one_series_and_its_missing_count() {
        let series_id = Uuid::new_v4();
        let request = SeriesRequest::for_one(series_id, None);

        let (sql, params) = build(&request);

        assert_bound(&sql, params.len());
        assert!(sql.contains("s.id = $1"), "the scope filters on the series");
        assert!(
            sql.contains("WHERE s.id = $1"),
            "the missing-count CTE is scoped to the same series"
        );
        assert!(sql.contains("LIMIT $3 OFFSET $4"), "one row at most");
    }

    #[test]
    fn the_favorites_scope_is_ordered_by_favorite_time_and_unpaged() {
        let request = SeriesRequest::for_favorites(Uuid::new_v4());

        let (sql, params) = build(&request);

        assert_bound(&sql, params.len());
        assert!(sql.contains("favorited_at DESC"));
        assert!(
            sql.contains(
                "s.id IN (SELECT series_id FROM series_user_favorites WHERE user_id = $1)"
            ),
            "the missing-count CTE borrows the favourite set"
        );
        assert!(
            !sql.contains("OFFSET"),
            "favourites return the whole set: no page window"
        );
        assert!(
            !sql.contains("latest_created_at"),
            "the All-scope order key must not leak into another scope"
        );
    }

    #[test]
    fn the_ongoing_scope_orders_by_activity_and_hides_shelved_series() {
        let request = SeriesRequest::for_ongoing(Uuid::new_v4(), 10);

        let (sql, params) = build(&request);

        assert_bound(&sql, params.len());
        assert!(sql.contains("ORDER BY last_read_at DESC NULLS LAST"));
        assert!(sql.contains("sus.created_at IS NULL"));
        assert!(
            sql.contains("s.id IN (SELECT series_id FROM active_series)"),
            "the missing-count CTE borrows the active set"
        );
        assert!(!sql.contains("latest_created_at"));
    }

    fn assert_bound(sql: &str, param_count: usize) {
        let max = placeholders(sql)
            .into_iter()
            .max()
            .expect("the query has at least the user placeholder");

        assert_eq!(
            max, param_count,
            "the highest placeholder ${max} must match the {param_count} bound parameters"
        );
        for index in 1..=max {
            assert!(
                placeholders(sql).contains(&index),
                "placeholder ${index} is never used"
            );
        }
    }

    fn placeholders(sql: &str) -> Vec<usize> {
        let bytes = sql.as_bytes();
        let mut found = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'$' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
                let start = index + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                found.push(sql[start..end].parse().expect("placeholder digits parse"));
                index = end;
            } else {
                index += 1;
            }
        }
        found
    }
}

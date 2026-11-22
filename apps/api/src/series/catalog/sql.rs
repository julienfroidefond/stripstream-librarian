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

/// Build the catalogue SQL and its bound parameters.
pub(super) fn build(request: &SeriesRequest) -> (String, Vec<Param>) {
    let mut binder = Binder::default();
    let filters = &request.filters;

    // The scope comes first so the missing-count CTE can reuse `$1` as its
    // library predicate.
    let (scope_where, library_param) = match &request.scope {
        SeriesScope::All { library_id } => match library_id {
            Some(library_id) => {
                let param = binder.push(Param::Uuid(*library_id));
                (format!("WHERE s.library_id = {param}"), Some(param))
            }
            None => ("WHERE TRUE".to_string(), None),
        },
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

    let title_order_clause = "lower(name) ASC";
    let order_clause = match request.sort {
        SeriesSort::Title => title_order_clause.to_string(),
        // For series without books, latest_created_at falls back to
        // s.created_at (see the series_counts CTE), so the value is never NULL.
        SeriesSort::Latest => "latest_created_at DESC".to_string(),
        SeriesSort::ReleaseDate => format!("start_year DESC NULLS LAST, {title_order_clause}"),
        SeriesSort::CommunityScore => {
            format!("community_score DESC NULLS LAST, {title_order_clause}")
        }
    };

    let missing_cte = helpers::build_missing_counts_cte(library_param.as_deref());
    let tome_counts = helpers::series_tome_counts("brp");

    let limit_param = binder.push(Param::Int(request.limit));
    let offset_param = binder.push(Param::Int((request.page - 1) * request.limit));

    let sql = format!(
        r#"
        WITH series_counts AS (
            SELECT
                s.id as series_id,
                s.name,
                s.library_id,
                {tome_counts},
                COALESCE(MAX(b.created_at), s.created_at) as latest_created_at
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND {user_param}::uuid IS NOT NULL AND brp.user_id = {user_param}
            {scope_where}
            GROUP BY s.id, s.name, s.library_id, s.created_at
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
                sus.created_at as shelved_at,
                sc.latest_created_at
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
        ),
        total AS (SELECT COUNT(*) AS total_count FROM filtered),
        page AS (
            SELECT * FROM filtered
            ORDER BY {order_clause}
            LIMIT {limit_param} OFFSET {offset_param}
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
            user_id: None,
        };

        let (sql, params) = build(&request);

        assert_bound(&sql, params.len());
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

//! Server-side genre browsing (ADR 0028).
//!
//! `GET /genres/browse` answers the whole genres page in one query set: the
//! filtered, paginated series list plus the facets (genre counts with a preview
//! cover, untagged count, total) of the library scope. Genre filters are an AND
//! (`genres @> $included`), exclusions an OR (`genres && $excluded`), both
//! served by the GIN index on `series.genres`.

use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, series::SeriesItem, state::AppState};

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 500;
/// Highest accepted `page`: far beyond any real catalogue (500 M series at the
/// max limit) while keeping `(page - 1) * limit` well inside `i64`.
const MAX_PAGE: i64 = 1_000_000;

/// Parsed `GET /genres/browse` query. Repeated keys (`genres=a&genres=b`) and
/// their bracketed form (`genres[]=a`) are both accepted.
#[derive(Debug, Default, PartialEq)]
pub struct GenreBrowseQuery {
    /// Genres every returned series must carry (AND).
    pub genres: Vec<String>,
    /// Genres no returned series may carry.
    pub exclude: Vec<String>,
    /// Library scope; empty means every library. Also scopes the facets.
    pub library_ids: Vec<Uuid>,
    /// Only series without any genre.
    pub untagged: bool,
    /// Case- and accent-insensitive partial match on the series name.
    pub q: Option<String>,
    pub page: i64,
    pub limit: i64,
}

impl GenreBrowseQuery {
    pub fn from_pairs(pairs: Vec<(String, String)>) -> Result<Self, ApiError> {
        let mut query = GenreBrowseQuery {
            page: 1,
            limit: DEFAULT_LIMIT,
            ..Default::default()
        };
        for (key, value) in pairs {
            let key = key.strip_suffix("[]").unwrap_or(&key);
            match key {
                "genres" | "genre" => push_distinct(&mut query.genres, value),
                "exclude" => push_distinct(&mut query.exclude, value),
                "library_ids" | "library_id" => {
                    let id = Uuid::parse_str(value.trim()).map_err(|_| {
                        ApiError::bad_request(format!("invalid library id '{value}'"))
                    })?;
                    if !query.library_ids.contains(&id) {
                        query.library_ids.push(id);
                    }
                }
                "untagged" => query.untagged = parse_bool(&value)?,
                "q" => {
                    let q = value.trim();
                    query.q = (!q.is_empty()).then(|| q.to_string());
                }
                "page" => {
                    let page = parse_int(key, &value)?;
                    if page > MAX_PAGE {
                        return Err(ApiError::bad_request(format!(
                            "page must be at most {MAX_PAGE}"
                        )));
                    }
                    query.page = page.max(1);
                }
                "limit" => query.limit = parse_int(key, &value)?.clamp(1, MAX_LIMIT),
                _ => {}
            }
        }
        Ok(query)
    }

    fn offset(&self) -> i64 {
        (self.page - 1).saturating_mul(self.limit)
    }

    fn library_scope(&self) -> Option<Vec<Uuid>> {
        (!self.library_ids.is_empty()).then(|| self.library_ids.clone())
    }
}

fn push_distinct(values: &mut Vec<String>, value: String) {
    let value = value.trim().to_string();
    if !value.is_empty() && !values.contains(&value) {
        values.push(value);
    }
}

fn parse_bool(value: &str) -> Result<bool, ApiError> {
    match value.trim() {
        "true" | "1" => Ok(true),
        "false" | "0" | "" => Ok(false),
        other => Err(ApiError::bad_request(format!(
            "invalid boolean '{other}' for untagged"
        ))),
    }
}

fn parse_int(key: &str, value: &str) -> Result<i64, ApiError> {
    value
        .trim()
        .parse()
        .map_err(|_| ApiError::bad_request(format!("invalid integer '{value}' for {key}")))
}

/// Preview cover of a genre: one representative series, preferring one with an image.
#[derive(Serialize, ToSchema)]
pub struct GenreCover {
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub name: String,
    #[schema(value_type = Option<String>)]
    pub first_book_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub first_book_updated_at: Option<DateTime<Utc>>,
    pub cover_url: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct GenreFacet {
    pub name: String,
    pub series_count: i64,
    pub cover: Option<GenreCover>,
}

/// Counts of the library scope, independent of the genre, exclusion and name filters.
#[derive(Serialize, ToSchema)]
pub struct GenreFacets {
    /// Genres present in the scope, alphabetical.
    pub genres: Vec<GenreFacet>,
    /// Series without any genre in the scope.
    pub untagged: i64,
    /// Series in the scope.
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
pub struct GenreBrowsePage {
    pub items: Vec<SeriesItem>,
    /// Series matching every filter (all pages).
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub facets: GenreFacets,
}

/// Browse series by genres with facets, in one request
#[utoipa::path(
    get,
    path = "/genres/browse",
    tag = "genres",
    params(
        ("genres" = Option<Vec<String>>, Query, description = "Repeatable. Series must carry every listed genre (AND, exact match)"),
        ("exclude" = Option<Vec<String>>, Query, description = "Repeatable. Series carrying any listed genre are left out"),
        ("library_ids" = Option<Vec<String>>, Query, description = "Repeatable. Library scope for the items and the facets (default: every library)"),
        ("untagged" = Option<bool>, Query, description = "Only series without any genre"),
        ("q" = Option<String>, Query, description = "Filter by series name (case- and accent-insensitive, partial match)"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1, max 1000000)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 500, default 100)"),
    ),
    responses(
        (status = 200, body = GenreBrowsePage),
        (status = 400, description = "Invalid query"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn browse_genres(
    State(state): State<AppState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<GenreBrowsePage>, ApiError> {
    let query = GenreBrowseQuery::from_pairs(pairs)?;
    Ok(Json(browse(&state.pool, &query).await?))
}

pub async fn browse(pool: &PgPool, query: &GenreBrowseQuery) -> Result<GenreBrowsePage, ApiError> {
    let ((items, total), facets) =
        tokio::try_join!(load_page(pool, query), load_facets(pool, query))?;
    Ok(GenreBrowsePage {
        items,
        total,
        page: query.page,
        limit: query.limit,
        facets,
    })
}

/// SQL predicate shared by the page and its total; `$1..$5` are bound by `load_page`.
const MATCHED_SERIES: &str = r#"
    SELECT s.id, s.name, s.library_id, s.status, s.cover_url, s.start_year,
           s.genres, s.authors, s.description
    FROM series s
    WHERE ($1::uuid[] IS NULL OR s.library_id = ANY($1))
      AND (NOT $2::bool OR cardinality(s.genres) = 0)
      AND s.genres @> $3::text[]
      AND NOT (s.genres && $4::text[])
      AND ($5::text IS NULL OR norm_text(s.name) LIKE norm_text($5))
"#;

async fn load_page(
    pool: &PgPool,
    query: &GenreBrowseQuery,
) -> Result<(Vec<SeriesItem>, i64), ApiError> {
    let sql = format!(
        r#"
        WITH matched AS ({MATCHED_SERIES}),
        total AS (SELECT COUNT(*)::bigint AS total_count FROM matched),
        page AS (
            SELECT * FROM matched
            ORDER BY lower(name), id
            LIMIT $6 OFFSET $7
        )
        SELECT
            p.id AS series_id, p.name, p.library_id, p.status AS series_status,
            p.cover_url, p.start_year, p.genres, p.authors, p.description,
            bc.book_count, fb.id AS first_book_id, fb.updated_at AS first_book_updated_at,
            t.total_count
        FROM total t
        LEFT JOIN page p ON TRUE
        LEFT JOIN LATERAL (
            SELECT CASE WHEN COUNT(*) FILTER (WHERE b.volume_type <> 'hs') > 0
                        THEN COUNT(*) FILTER (WHERE b.volume_type <> 'hs')
                        ELSE COUNT(*)
                   END::bigint AS book_count
            FROM books b WHERE b.series_id = p.id
        ) bc ON TRUE
        LEFT JOIN LATERAL (
            SELECT b.id, b.updated_at FROM books b
            WHERE b.series_id = p.id
            ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                     b.volume NULLS LAST, b.title
            LIMIT 1
        ) fb ON TRUE
        ORDER BY lower(p.name), p.id
        "#
    );
    let q_pattern = query.q.as_deref().map(|q| format!("%{q}%"));

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(query.library_scope())
        .bind(query.untagged)
        .bind(&query.genres)
        .bind(&query.exclude)
        .bind(q_pattern)
        .bind(query.limit)
        .bind(query.offset())
        .fetch_all(pool)
        .await?;

    let total = rows.first().map(|r| r.get("total_count")).unwrap_or(0);
    let items = rows
        .iter()
        .filter_map(|row| {
            let series_id: Option<Uuid> = row.get("series_id");
            Some(SeriesItem {
                series_id: series_id?,
                name: row.get("name"),
                library_id: row.get("library_id"),
                series_status: row.get("series_status"),
                cover_url: row.get("cover_url"),
                book_count: row.get::<Option<i64>, _>("book_count").unwrap_or(0),
                books_read_count: 0,
                last_read_at: None,
                missing_count: None,
                metadata_provider: None,
                anilist_id: None,
                anilist_url: None,
                first_book_id: row.get("first_book_id"),
                first_book_updated_at: row.get("first_book_updated_at"),
                start_year: row.get("start_year"),
                genres: row.get("genres"),
                authors: row.get("authors"),
                description: row.get("description"),
                user_rating: None,
                community_score: None,
                shelved_at: None,
            })
        })
        .collect();
    Ok((items, total))
}

async fn load_facets(pool: &PgPool, query: &GenreBrowseQuery) -> Result<GenreFacets, ApiError> {
    let scope = query.library_scope();

    let genre_rows = sqlx::query(
        r#"
        WITH scope AS (
            SELECT s.id, s.name, s.cover_url, s.genres
            FROM series s
            WHERE ($1::uuid[] IS NULL OR s.library_id = ANY($1))
              AND cardinality(s.genres) > 0
        ),
        tagged AS (
            SELECT DISTINCT sc.id, sc.name, sc.cover_url, g.genre
            FROM scope sc CROSS JOIN LATERAL unnest(sc.genres) AS g(genre)
        ),
        counts AS (
            SELECT genre, COUNT(*)::bigint AS series_count FROM tagged GROUP BY genre
        ),
        pick AS (
            SELECT DISTINCT ON (t.genre) t.genre, t.id, t.name, t.cover_url
            FROM tagged t
            ORDER BY t.genre,
                     (EXISTS (SELECT 1 FROM books b WHERE b.series_id = t.id)
                      OR COALESCE(t.cover_url, '') <> '') DESC,
                     lower(t.name), t.id
        )
        SELECT c.genre, c.series_count,
               p.id AS cover_series_id, p.name AS cover_name, p.cover_url,
               fb.id AS first_book_id, fb.updated_at AS first_book_updated_at
        FROM counts c
        JOIN pick p ON p.genre = c.genre
        LEFT JOIN LATERAL (
            SELECT b.id, b.updated_at FROM books b
            WHERE b.series_id = p.id
            ORDER BY CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                     b.volume NULLS LAST, b.title
            LIMIT 1
        ) fb ON TRUE
        ORDER BY c.genre
        "#,
    )
    .bind(&scope)
    .fetch_all(pool);

    let totals = sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS total,
               COUNT(*) FILTER (WHERE cardinality(genres) = 0)::bigint AS untagged
        FROM series
        WHERE ($1::uuid[] IS NULL OR library_id = ANY($1))
        "#,
    )
    .bind(&scope)
    .fetch_one(pool);

    let (genre_rows, totals) = tokio::try_join!(genre_rows, totals)?;

    let genres = genre_rows
        .iter()
        .map(|row| {
            let cover_url: Option<String> = row.get("cover_url");
            let first_book_id: Option<Uuid> = row.get("first_book_id");
            let has_image =
                first_book_id.is_some() || cover_url.as_deref().is_some_and(|u| !u.is_empty());
            GenreFacet {
                name: row.get("genre"),
                series_count: row.get("series_count"),
                cover: has_image.then(|| GenreCover {
                    series_id: row.get("cover_series_id"),
                    name: row.get("cover_name"),
                    first_book_id,
                    first_book_updated_at: row.get("first_book_updated_at"),
                    cover_url,
                }),
            }
        })
        .collect();

    Ok(GenreFacets {
        genres,
        untagged: totals.get("untagged"),
        total: totals.get("total"),
    })
}

#[cfg(test)]
#[path = "../tests/genres_browse.rs"]
mod tests;

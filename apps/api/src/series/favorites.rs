use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

use super::SeriesItem;

#[utoipa::path(
    get,
    path = "/favorites",
    tag = "series",
    responses((status = 200, body = Vec<SeriesItem>)),
    security(("Bearer" = []))
)]
pub async fn list_favorites(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let user_id = user.map(|u| u.0.user_id).ok_or_else(|| {
        ApiError::bad_request("no user selected — choose a user in the backoffice settings")
    })?;

    let rows = sqlx::query(
        r#"
        SELECT s.name, s.id AS series_id, s.library_id, s.status AS series_status,
               COUNT(b.id)::bigint AS book_count,
               COUNT(brp.book_id) FILTER (WHERE brp.status = 'read')::bigint AS books_read_count,
               fb.id AS first_book_id, fb.updated_at AS first_book_updated_at,
               ml.provider AS metadata_provider, asl.anilist_id, asl.anilist_url,
               s.cover_url, s.start_year, COALESCE(s.genres, ARRAY[]::text[]) AS genres,
               COALESCE(s.authors, ARRAY[]::text[]) AS authors, s.description,
               sur.rating AS user_rating
        FROM series_user_favorites f
        JOIN series s ON s.id = f.series_id
        LEFT JOIN books b ON b.series_id = s.id
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $1
        LEFT JOIN LATERAL (
            SELECT id, updated_at FROM books
            WHERE series_id = s.id
            ORDER BY CASE WHEN volume_type = 'regular' THEN 0 ELSE 1 END, volume NULLS LAST, title
            LIMIT 1
        ) fb ON TRUE
        LEFT JOIN LATERAL (
            SELECT provider FROM external_metadata_links
            WHERE series_id = s.id AND status = 'approved'
            ORDER BY is_primary DESC, created_at DESC LIMIT 1
        ) ml ON TRUE
        LEFT JOIN anilist_series_links asl ON asl.series_id = s.id AND asl.provider = 'anilist'
        LEFT JOIN series_user_ratings sur ON sur.series_id = s.id AND sur.user_id = $1
        WHERE f.user_id = $1
          AND NOT EXISTS (
              SELECT 1 FROM user_genre_restrictions ugr
              WHERE ugr.user_id = $1 AND ugr.genre = ANY(s.genres)
          )
        GROUP BY f.created_at, s.id, s.name, s.library_id, s.status, fb.id, fb.updated_at,
                 ml.provider, asl.anilist_id, asl.anilist_url, sur.rating
        ORDER BY f.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.iter().map(series_item_from_row).collect()))
}

#[utoipa::path(
    put,
    path = "/series/{series_id}/favorite",
    tag = "series",
    security(("Bearer" = []))
)]
pub async fn add_favorite(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = user
        .map(|u| u.0.user_id)
        .ok_or_else(|| ApiError::bad_request("no user selected"))?;
    sqlx::query(
        "INSERT INTO series_user_favorites (user_id, series_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(series_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/series/{series_id}/favorite",
    tag = "series",
    security(("Bearer" = []))
)]
pub async fn remove_favorite(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = user
        .map(|u| u.0.user_id)
        .ok_or_else(|| ApiError::bad_request("no user selected"))?;
    sqlx::query("DELETE FROM series_user_favorites WHERE user_id = $1 AND series_id = $2")
        .bind(user_id)
        .bind(series_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/series/{series_id}/favorite",
    tag = "series",
    security(("Bearer" = []))
)]
pub async fn is_favorite(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<bool>, ApiError> {
    let user_id = user
        .map(|u| u.0.user_id)
        .ok_or_else(|| ApiError::bad_request("no user selected"))?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM series_user_favorites WHERE user_id = $1 AND series_id = $2)",
    )
    .bind(user_id)
    .bind(series_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(exists))
}

fn series_item_from_row(row: &sqlx::postgres::PgRow) -> SeriesItem {
    SeriesItem {
        name: row.get("name"),
        series_id: row.get("series_id"),
        book_count: row.get("book_count"),
        books_read_count: row.get("books_read_count"),
        first_book_id: row.get("first_book_id"),
        first_book_updated_at: row.get("first_book_updated_at"),
        library_id: row.get("library_id"),
        series_status: row.get("series_status"),
        missing_count: None,
        metadata_provider: row.get("metadata_provider"),
        anilist_id: row.get("anilist_id"),
        anilist_url: row.get("anilist_url"),
        cover_url: row.get("cover_url"),
        start_year: row.get("start_year"),
        genres: row.get("genres"),
        authors: row.get("authors"),
        description: row.get("description"),
        user_rating: row.get("user_rating"),
        community_score: None,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{atomic::AtomicU64, Arc},
    };

    use tokio::sync::{Mutex, RwLock, Semaphore};

    use super::*;
    use crate::series::ratings::{delete_series_rating, set_series_rating, SetRatingRequest};
    use crate::state::{
        DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
    };

    fn test_state(pool: sqlx::PgPool) -> AppState {
        AppState {
            pool,
            bootstrap_token: Arc::from("test-token"),
            page_cache: Arc::new(Mutex::new(crate::state::PageCache::new(1))),
            disk_cache_stats: Arc::new(Mutex::new(None::<DiskCacheStatsSnapshot>)),
            page_render_locks: Arc::new(PageRenderLocks::new(1)),
            page_render_limit: Arc::new(Semaphore::new(1)),
            metrics: Arc::new(Metrics {
                requests_total: AtomicU64::new(0),
                page_cache_hits: AtomicU64::new(0),
                page_cache_misses: AtomicU64::new(0),
            }),
            read_rate_limit: Arc::new(Mutex::new(ReadRateLimit::new())),
            settings: Arc::new(RwLock::new(DynamicSettings::default())),
            prowlarr_fetch_lock: Arc::new(Mutex::new(())),
            pending_tg_auth: Arc::new(Mutex::new(None)),
            telegram_download_limit: Arc::new(Semaphore::new(1)),
            telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn create_user(pool: &sqlx::PgPool, username: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
            .bind(username)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn create_series(pool: &sqlx::PgPool, name: &str, genres: &[&str]) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind(format!("library-{name}"))
            .bind(format!("/libraries/{library_id}"))
            .execute(pool)
            .await
            .unwrap();

        sqlx::query_scalar(
            "INSERT INTO series (id, library_id, name, genres) VALUES (gen_random_uuid(), $1, $2, $3) RETURNING id",
        )
        .bind(library_id)
        .bind(name)
        .bind(genres)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn auth_user(user_id: Uuid) -> Option<Extension<AuthUser>> {
        Some(Extension(AuthUser { user_id }))
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn add_check_and_remove_favorite_are_idempotent(pool: sqlx::PgPool) {
        let state = test_state(pool.clone());
        let user_id = create_user(&pool, "favorites-user").await;
        let series_id = create_series(&pool, "Favorite series", &[]).await;

        for _ in 0..2 {
            let status = add_favorite(State(state.clone()), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
            assert_eq!(status, StatusCode::NO_CONTENT);
        }

        let Json(favorite_before_removal) =
            is_favorite(State(state.clone()), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
        assert!(favorite_before_removal);

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM series_user_favorites WHERE user_id = $1 AND series_id = $2",
        )
        .bind(user_id)
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        remove_favorite(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();
        remove_favorite(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();

        let Json(favorite_after_removal) =
            is_favorite(State(state), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
        assert!(!favorite_after_removal);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn favorites_are_scoped_to_the_selected_user(pool: sqlx::PgPool) {
        let state = test_state(pool.clone());
        let first_user = create_user(&pool, "first-user").await;
        let second_user = create_user(&pool, "second-user").await;
        let series_id = create_series(&pool, "Private favorite", &[]).await;

        add_favorite(State(state.clone()), auth_user(first_user), Path(series_id))
            .await
            .unwrap();

        let Json(first_favorites) = list_favorites(State(state.clone()), auth_user(first_user))
            .await
            .unwrap();
        let Json(second_favorites) = list_favorites(State(state), auth_user(second_user))
            .await
            .unwrap();

        assert_eq!(first_favorites.len(), 1);
        assert_eq!(first_favorites[0].series_id, series_id);
        assert!(second_favorites.is_empty());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn read_user_identity_can_mutate_rating(pool: sqlx::PgPool) {
        let state = test_state(pool.clone());
        let user_id = create_user(&pool, "rating-read-user").await;
        let series_id = create_series(&pool, "Rated series", &[]).await;

        let status = set_series_rating(
            State(state.clone()),
            auth_user(user_id),
            Path(series_id),
            Json(SetRatingRequest { rating: 9 }),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT);

        let rating: Option<i16> = sqlx::query_scalar(
            "SELECT rating FROM series_user_ratings WHERE user_id = $1 AND series_id = $2",
        )
        .bind(user_id)
        .bind(series_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(rating, Some(9));

        let status = delete_series_rating(State(state), auth_user(user_id), Path(series_id))
            .await
            .unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT);

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM series_user_ratings WHERE user_id = $1 AND series_id = $2",
        )
        .bind(user_id)
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn list_favorites_respects_genre_restrictions(pool: sqlx::PgPool) {
        let state = test_state(pool.clone());
        let user_id = create_user(&pool, "restricted-user").await;
        let visible_id = create_series(&pool, "Visible series", &["Adventure"]).await;
        let restricted_id = create_series(&pool, "Restricted series", &["Mature"]).await;

        for series_id in [visible_id, restricted_id] {
            add_favorite(State(state.clone()), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO user_genre_restrictions (user_id, genre) VALUES ($1, 'Mature')")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        let Json(favorites) = list_favorites(State(state), auth_user(user_id))
            .await
            .unwrap();

        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].series_id, visible_id);
    }
}

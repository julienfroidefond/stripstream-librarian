use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

fn require_user(user: Option<Extension<AuthUser>>) -> Result<Uuid, ApiError> {
    user.map(|u| u.0.user_id).ok_or_else(|| {
        ApiError::bad_request("no user selected — choose a user in the backoffice settings")
    })
}

// ─── Shelf status ────────────────────────────────────────────────────────────

/// Whether the current user put this series aside ("Mettre de côté")
#[utoipa::path(
    get,
    path = "/series/{series_id}/shelf",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    responses(
        (status = 200, body = bool),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn is_shelved(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<bool>, ApiError> {
    let user_id = require_user(user)?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM series_user_shelves WHERE user_id = $1 AND series_id = $2)",
    )
    .bind(user_id)
    .bind(series_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(exists))
}

// ─── Shelve / unshelve ───────────────────────────────────────────────────────

/// Put a series aside for the current user (hidden from ongoing lists)
#[utoipa::path(
    put,
    path = "/series/{series_id}/shelf",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    responses(
        (status = 204, description = "Series shelved"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn shelve_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = require_user(user)?;
    sqlx::query(
        "INSERT INTO series_user_shelves (user_id, series_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(series_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Resume a series put aside by the current user
#[utoipa::path(
    delete,
    path = "/series/{series_id}/shelf",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    responses(
        (status = 204, description = "Series resumed"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn unshelve_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = require_user(user)?;
    sqlx::query("DELETE FROM series_user_shelves WHERE user_id = $1 AND series_id = $2")
        .bind(user_id)
        .bind(series_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::ongoing::{ongoing_books, ongoing_series};
    use crate::series::{add_favorite, get_series_by_id, list_favorites, OngoingQuery};
    use crate::state::test_state;
    use axum::extract::Query;
    use chrono::{DateTime, Utc};
    use sqlx::PgPool;

    async fn create_library(pool: &PgPool, name: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO libraries (id, name, root_path) \
             VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(name)
        .bind(format!("/libraries/{}-{}", name, Uuid::new_v4()))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn create_user(pool: &PgPool, username: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO users (id, username) VALUES (gen_random_uuid(), $1) RETURNING id",
        )
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn create_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO series (id, library_id, name) \
             VALUES (gen_random_uuid(), $1, $2) RETURNING id",
        )
        .bind(library_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn create_book(
        pool: &PgPool,
        library_id: Uuid,
        series_id: Uuid,
        title: &str,
        volume: i32,
    ) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO books \
                 (id, library_id, kind, title, series_id, volume, volume_type, authors, page_count) \
             VALUES (gen_random_uuid(), $1, 'comic', $2, $3, $4, 'regular', '{}', 10) RETURNING id",
        )
        .bind(library_id)
        .bind(title)
        .bind(series_id)
        .bind(volume)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn set_read(pool: &PgPool, book_id: Uuid, user_id: Uuid, last_read_at: DateTime<Utc>) {
        sqlx::query(
            "INSERT INTO book_reading_progress \
                 (book_id, user_id, status, last_read_at, updated_at) \
             VALUES ($1, $2, 'read', $3, $3)",
        )
        .bind(book_id)
        .bind(user_id)
        .bind(last_read_at)
        .execute(pool)
        .await
        .unwrap();
    }

    fn auth_user(user_id: Uuid) -> Option<Extension<AuthUser>> {
        Some(Extension(AuthUser { user_id }))
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn shelve_check_and_unshelve_are_idempotent(pool: PgPool) {
        let state = test_state(pool.clone());
        let user_id = create_user(&pool, "shelf-user").await;
        let library_id = create_library(&pool, "shelf-library").await;
        let series_id = create_series(&pool, library_id, "Shelved series").await;

        for _ in 0..2 {
            let status = shelve_series(State(state.clone()), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
            assert_eq!(status, StatusCode::NO_CONTENT);
        }

        let Json(shelved) = is_shelved(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();
        assert!(shelved);

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM series_user_shelves WHERE user_id = $1 AND series_id = $2",
        )
        .bind(user_id)
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        unshelve_series(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();
        unshelve_series(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();

        let Json(shelved) = is_shelved(State(state), auth_user(user_id), Path(series_id))
            .await
            .unwrap();
        assert!(!shelved);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn shelved_series_are_hidden_from_ongoing_lists(pool: PgPool) {
        let state = test_state(pool.clone());
        let reader = create_user(&pool, "shelf-reader").await;
        let other = create_user(&pool, "shelf-other").await;
        let library_id = create_library(&pool, "shelf-library").await;
        let series_id = create_series(&pool, library_id, "Ongoing series").await;
        let vol1 = create_book(&pool, library_id, series_id, "Vol 1", 1).await;
        let _vol2 = create_book(&pool, library_id, series_id, "Vol 2", 2).await;

        let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        set_read(&pool, vol1, reader, read_at).await;
        set_read(&pool, vol1, other, read_at).await;

        let Json(series_before) = ongoing_series(
            State(state.clone()),
            auth_user(reader),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert_eq!(series_before.len(), 1);

        shelve_series(State(state.clone()), auth_user(reader), Path(series_id))
            .await
            .unwrap();

        let Json(series_after) = ongoing_series(
            State(state.clone()),
            auth_user(reader),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert!(series_after.is_empty());

        let Json(books_after) = ongoing_books(
            State(state.clone()),
            auth_user(reader),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert!(books_after.is_empty());

        // Shelving is per user: another reader still sees the series.
        let Json(other_series) = ongoing_series(
            State(state.clone()),
            auth_user(other),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert_eq!(other_series.len(), 1);

        let Json(other_books) = ongoing_books(
            State(state.clone()),
            auth_user(other),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert_eq!(other_books.len(), 1);

        // Resuming the series puts it back in the ongoing lists.
        unshelve_series(State(state.clone()), auth_user(reader), Path(series_id))
            .await
            .unwrap();

        let Json(series_resumed) = ongoing_series(
            State(state.clone()),
            auth_user(reader),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert_eq!(series_resumed.len(), 1);

        let Json(books_resumed) = ongoing_books(
            State(state),
            auth_user(reader),
            Query(OngoingQuery { limit: None }),
        )
        .await
        .unwrap();
        assert_eq!(books_resumed.len(), 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_detail_and_favorites_expose_shelved_at(pool: PgPool) {
        let state = test_state(pool.clone());
        let user_id = create_user(&pool, "shelf-detail-user").await;
        let library_id = create_library(&pool, "shelf-detail-library").await;
        let series_id = create_series(&pool, library_id, "Detail series").await;
        create_book(&pool, library_id, series_id, "Vol 1", 1).await;

        add_favorite(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();
        shelve_series(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();

        let Json(detail) =
            get_series_by_id(State(state.clone()), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
        assert!(detail.shelved_at.is_some());

        let Json(favorites) = list_favorites(State(state.clone()), auth_user(user_id))
            .await
            .unwrap();
        assert_eq!(favorites.len(), 1);
        assert!(favorites[0].shelved_at.is_some());

        unshelve_series(State(state.clone()), auth_user(user_id), Path(series_id))
            .await
            .unwrap();

        let Json(detail) =
            get_series_by_id(State(state.clone()), auth_user(user_id), Path(series_id))
                .await
                .unwrap();
        assert!(detail.shelved_at.is_none());

        let Json(favorites) = list_favorites(State(state), auth_user(user_id))
            .await
            .unwrap();
        assert_eq!(favorites.len(), 1);
        assert!(favorites[0].shelved_at.is_none());
    }
}

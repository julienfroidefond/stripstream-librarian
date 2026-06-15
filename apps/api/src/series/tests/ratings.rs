use crate::series::ratings::{ProviderRating, SeriesRatingsResponse};
use uuid::Uuid;

// ─── Unit: response serialization ────────────────────────────────────────────

#[test]
fn series_ratings_response_serializes() {
    let resp = SeriesRatingsResponse {
        user_rating: Some(8),
        anilist_pulled_rating: Some(7.5),
        provider_ratings: vec![
            ProviderRating {
                provider: "anilist".to_string(),
                rating: 78.0,
                rating_scale: 100.0,
                rating_count: Some(15234),
            },
            ProviderRating {
                provider: "senscritique".to_string(),
                rating: 7.8,
                rating_scale: 10.0,
                rating_count: None,
            },
        ],
    };

    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["user_rating"], 8);
    assert_eq!(json["anilist_pulled_rating"], 7.5);
    assert_eq!(json["provider_ratings"][0]["provider"], "anilist");
    assert_eq!(json["provider_ratings"][0]["rating"], 78.0);
    assert_eq!(json["provider_ratings"][0]["rating_scale"], 100.0);
    assert_eq!(json["provider_ratings"][0]["rating_count"], 15234);
    assert!(json["provider_ratings"][1]["rating_count"].is_null());
}

#[test]
fn series_ratings_response_null_user_rating() {
    let resp = SeriesRatingsResponse {
        user_rating: None,
        anilist_pulled_rating: None,
        provider_ratings: vec![],
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json["user_rating"].is_null());
    assert!(json["anilist_pulled_rating"].is_null());
    assert!(json["provider_ratings"].as_array().unwrap().is_empty());
}

// ─── Integration: series_user_ratings upsert ─────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn upsert_series_user_rating(pool: sqlx::PgPool) {
    // Setup: library + series + user
    let lib_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')",
    )
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let series_id: Uuid = sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, 'My Series') RETURNING id",
    )
    .bind(lib_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role) VALUES ($1, 'testuser', 'hash', 'admin')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert
    sqlx::query(
        r#"INSERT INTO series_user_ratings (user_id, series_id, rating) VALUES ($1, $2, $3)"#,
    )
    .bind(user_id)
    .bind(series_id)
    .bind(8i16)
    .execute(&pool)
    .await
    .unwrap();

    let rating: i16 = sqlx::query_scalar(
        "SELECT rating FROM series_user_ratings WHERE user_id = $1 AND series_id = $2",
    )
    .bind(user_id)
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rating, 8);

    // Update via ON CONFLICT
    sqlx::query(
        r#"
        INSERT INTO series_user_ratings (user_id, series_id, rating)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, series_id) DO UPDATE SET rating = EXCLUDED.rating, updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(series_id)
    .bind(10i16)
    .execute(&pool)
    .await
    .unwrap();

    let updated: i16 = sqlx::query_scalar(
        "SELECT rating FROM series_user_ratings WHERE user_id = $1 AND series_id = $2",
    )
    .bind(user_id)
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(updated, 10);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn cascade_delete_user_removes_ratings(pool: sqlx::PgPool) {
    let lib_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')",
    )
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let series_id: Uuid = sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, 'S2') RETURNING id",
    )
    .bind(lib_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role) VALUES ($1, 'deluser', 'hash', 'admin')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO series_user_ratings (user_id, series_id, rating) VALUES ($1, $2, 5)")
        .bind(user_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    // Delete user → cascade
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM series_user_ratings WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

use uuid::Uuid;

async fn create_library(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(format!("lib-{id}"))
        .bind(format!("/libraries/lib-{id}"))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn create_series(
    pool: &sqlx::PgPool,
    lib_id: Uuid,
    name: &str,
    authors: &[&str],
    genres: &[&str],
    publishers: &[&str],
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO series (id, library_id, name, authors, genres, publishers) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(id)
    .bind(lib_id)
    .bind(name)
    .bind(authors)
    .bind(genres)
    .bind(publishers)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn fetch_related(
    pool: &sqlx::PgPool,
    series_id: Uuid,
    limit: i64,
) -> Vec<(Uuid, i64, Vec<String>)> {
    let rows = sqlx::query(
        r#"
        WITH ref AS (
            SELECT authors, genres, publishers FROM series WHERE id = $1
        ),
        author_scores AS (
            SELECT s.id AS series_id, COUNT(*)::bigint AS cnt
            FROM series s, ref, unnest(s.authors) sa, unnest(ref.authors) ra
            WHERE s.id != $1 AND sa = ra
            GROUP BY s.id
        ),
        genre_scores AS (
            SELECT s.id AS series_id, COUNT(*)::bigint AS cnt
            FROM series s, ref, unnest(s.genres) sg, unnest(ref.genres) rg
            WHERE s.id != $1 AND sg = rg
            GROUP BY s.id
        )
        SELECT
            s.id AS series_id,
            (
                COALESCE(asc_.cnt, 0) * 3 +
                COALESCE(gsc_.cnt, 0) * 2 +
                CASE WHEN s.publishers && ref.publishers THEN 1 ELSE 0 END
            ) AS score,
            (asc_.cnt IS NOT NULL) AS has_same_author,
            (gsc_.cnt IS NOT NULL) AS has_same_genre,
            (s.publishers && ref.publishers) AS has_same_publisher
        FROM series s
        CROSS JOIN ref
        LEFT JOIN author_scores asc_ ON asc_.series_id = s.id
        LEFT JOIN genre_scores gsc_ ON gsc_.series_id = s.id
        WHERE s.id != $1
          AND (s.authors && ref.authors OR s.genres && ref.genres OR s.publishers && ref.publishers)
        ORDER BY score DESC
        LIMIT $2
    "#,
    )
    .bind(series_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap();

    use sqlx::Row;
    rows.into_iter()
        .map(|row| {
            let mut reasons = Vec::new();
            if row.get::<bool, _>("has_same_author") {
                reasons.push("same_author".to_string());
            }
            if row.get::<bool, _>("has_same_genre") {
                reasons.push("same_genre".to_string());
            }
            if row.get::<bool, _>("has_same_publisher") {
                reasons.push("same_publisher".to_string());
            }
            (row.get("series_id"), row.get::<i64, _>("score"), reasons)
        })
        .collect()
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_same_author(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let s1 = create_series(&pool, lib, "Naruto", &["Kishimoto"], &[], &[]).await;
    let s2 = create_series(&pool, lib, "Boruto", &["Kishimoto"], &[], &[]).await;
    let _ = create_series(&pool, lib, "One Piece", &["Oda"], &[], &[]).await;

    let results = fetch_related(&pool, s1, 10).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, s2);
    assert_eq!(results[0].1, 3); // score = 3 (1 shared author × 3)
    assert!(results[0].2.contains(&"same_author".to_string()));
    assert!(!results[0].2.contains(&"same_genre".to_string()));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_same_genre(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let s1 = create_series(&pool, lib, "Akira", &["Otomo"], &["Manga", "Sci-Fi"], &[]).await;
    let s2 = create_series(
        &pool,
        lib,
        "Ghost in the Shell",
        &["Shirow"],
        &["Manga", "Cyberpunk"],
        &[],
    )
    .await;
    let _ = create_series(
        &pool,
        lib,
        "Dragon Ball",
        &["Toriyama"],
        &["Adventure"],
        &[],
    )
    .await;

    let results = fetch_related(&pool, s1, 10).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, s2);
    assert_eq!(results[0].1, 2); // score = 2 (1 shared genre × 2)
    assert!(results[0].2.contains(&"same_genre".to_string()));
    assert!(!results[0].2.contains(&"same_author".to_string()));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_same_publisher(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let s1 = create_series(&pool, lib, "Bleach", &["Kubo"], &[], &["Shueisha"]).await;
    let s2 = create_series(
        &pool,
        lib,
        "Hunter x Hunter",
        &["Togashi"],
        &[],
        &["Shueisha"],
    )
    .await;
    let _ = create_series(&pool, lib, "Berserk", &["Miura"], &[], &["Dark Horse"]).await;

    let results = fetch_related(&pool, s1, 10).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, s2);
    assert_eq!(results[0].1, 1); // score = 1 (shared publisher)
    assert!(results[0].2.contains(&"same_publisher".to_string()));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_scoring_order(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let base = create_series(&pool, lib, "Base", &["Author A"], &["Genre X"], &["Pub Z"]).await;
    // Score 3: same author only
    let by_author = create_series(&pool, lib, "ByAuthor", &["Author A"], &[], &[]).await;
    // Score 2: same genre only
    let by_genre = create_series(&pool, lib, "ByGenre", &[], &["Genre X"], &[]).await;
    // Score 1: same publisher only
    let by_pub = create_series(&pool, lib, "ByPub", &[], &[], &["Pub Z"]).await;
    // Score 5: same author + genre (3+2)
    let by_both = create_series(&pool, lib, "ByBoth", &["Author A"], &["Genre X"], &[]).await;

    let results = fetch_related(&pool, base, 10).await;
    let ids: Vec<Uuid> = results.iter().map(|r| r.0).collect();
    let scores: Vec<i64> = results.iter().map(|r| r.1).collect();

    assert_eq!(ids[0], by_both); // score 5
    assert_eq!(scores[0], 5);
    assert_eq!(ids[1], by_author); // score 3
    assert_eq!(scores[1], 3);
    assert_eq!(ids[2], by_genre); // score 2
    assert_eq!(scores[2], 2);
    assert_eq!(ids[3], by_pub); // score 1
    assert_eq!(scores[3], 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_excludes_self(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let s1 = create_series(
        &pool,
        lib,
        "Solo Series",
        &["Author A"],
        &["Genre X"],
        &["Pub Z"],
    )
    .await;

    let results = fetch_related(&pool, s1, 10).await;
    assert!(results.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_empty_when_no_match(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let s1 = create_series(&pool, lib, "Series A", &["Author A"], &["Genre X"], &[]).await;
    let _ = create_series(&pool, lib, "Series B", &["Author B"], &["Genre Y"], &[]).await;

    let results = fetch_related(&pool, s1, 10).await;
    assert!(results.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_limit_respected(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let base = create_series(&pool, lib, "Base", &["Shared"], &[], &[]).await;
    for i in 0..8 {
        create_series(&pool, lib, &format!("Series {i}"), &["Shared"], &[], &[]).await;
    }

    let results = fetch_related(&pool, base, 3).await;
    assert_eq!(results.len(), 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn related_multiple_shared_authors_multiply_score(pool: sqlx::PgPool) {
    let lib = create_library(&pool).await;
    let base = create_series(&pool, lib, "Base", &["A1", "A2", "A3"], &[], &[]).await;
    let two = create_series(&pool, lib, "TwoAuthors", &["A1", "A2"], &[], &[]).await;
    let one = create_series(&pool, lib, "OneAuthor", &["A1"], &[], &[]).await;

    let results = fetch_related(&pool, base, 10).await;
    let scores: std::collections::HashMap<Uuid, i64> = results.iter().map(|r| (r.0, r.1)).collect();

    assert_eq!(scores[&two], 6); // 2 shared authors × 3
    assert_eq!(scores[&one], 3); // 1 shared author × 3
                                 // two comes before one
    assert_eq!(results[0].0, two);
    assert_eq!(results[1].0, one);
}

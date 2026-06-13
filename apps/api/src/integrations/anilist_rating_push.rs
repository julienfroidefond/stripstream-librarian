use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::anilist::{anilist_graphql, load_anilist_settings};

const SAVE_SCORE_MUTATION: &str = r#"
    mutation SaveScore($mediaId: Int, $score: Float) {
        SaveMediaListEntry(mediaId: $mediaId, score: $score) {
            id
            score
        }
    }
"#;

/// Push a local rating (1-10 half-star scale) to AniList as a POINT_100 score.
/// Silently no-ops if AniList is not configured or the series has no AniList link.
/// Should be called via tokio::spawn so it doesn't block the HTTP response.
pub async fn push_rating_to_anilist(
    pool: &PgPool,
    series_id: Uuid,
    local_user_id: Uuid,
    rating_1_10: i16,
) {
    if let Err(e) =
        push_rating_inner(pool, series_id, local_user_id, rating_1_10).await
    {
        tracing::warn!("AniList rating push failed for series {series_id}: {e}");
    }
}

async fn push_rating_inner(
    pool: &PgPool,
    series_id: Uuid,
    _local_user_id: Uuid,
    rating_1_10: i16,
) -> Result<(), String> {
    let (token, _, _) = load_anilist_settings(pool)
        .await
        .map_err(|e| e.message)?;

    let row = sqlx::query(
        "SELECT anilist_id FROM anilist_series_links WHERE series_id = $1 LIMIT 1",
    )
    .bind(series_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let anilist_id: i32 = match row {
        Some(r) => r.get("anilist_id"),
        None => return Ok(()), // No AniList link for this series
    };

    // Convert 1-10 (half-star ×2) to POINT_100
    let score_100 = (rating_1_10 as f64) * 10.0;

    anilist_graphql(
        &token,
        SAVE_SCORE_MUTATION,
        serde_json::json!({
            "mediaId": anilist_id,
            "score": score_100,
        }),
    )
    .await
    .map_err(|e| e.message)?;

    // Update the cached user_score on the link
    let _ = sqlx::query(
        "UPDATE anilist_series_links SET user_score = $1 WHERE series_id = $2",
    )
    .bind(rating_1_10 as f64)
    .bind(series_id)
    .execute(pool)
    .await;

    Ok(())
}

// ─── Conversion helpers (tested) ─────────────────────────────────────────────

/// Convert a local 1-10 rating to an AniList score according to the user's scoreFormat.
/// AniList accepts score as a float regardless of format; the API normalizes it.
pub fn local_rating_to_anilist_score(rating_1_10: i16) -> f64 {
    (rating_1_10 as f64) * 10.0
}

/// Convert an AniList POINT_100 score to our local 1-10 scale (rounded, clamped).
pub fn anilist_score_to_local(score_100: f64) -> i16 {
    let normalised = score_100 / 10.0;
    (normalised.round() as i16).clamp(1, 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_to_anilist_roundtrip() {
        for r in 1i16..=10 {
            let score = local_rating_to_anilist_score(r);
            let back = anilist_score_to_local(score);
            assert_eq!(back, r, "roundtrip failed for rating {r}");
        }
    }

    #[test]
    fn anilist_edge_cases() {
        // score 0 → clamp to 1
        assert_eq!(anilist_score_to_local(0.0), 1);
        // score 100 → 10
        assert_eq!(anilist_score_to_local(100.0), 10);
        // score 75 → round(7.5) = 8
        assert_eq!(anilist_score_to_local(75.0), 8);
        // score 45 → round(4.5) = 5
        assert_eq!(anilist_score_to_local(45.0), 5);
    }

    #[test]
    fn local_to_anilist_values() {
        assert_eq!(local_rating_to_anilist_score(1), 10.0);
        assert_eq!(local_rating_to_anilist_score(5), 50.0);
        assert_eq!(local_rating_to_anilist_score(10), 100.0);
    }
}

use std::collections::{HashMap, HashSet};

use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::shared_sync;
use crate::metadata_providers;

// ---------------------------------------------------------------------------
// Approved links, provider by provider
// ---------------------------------------------------------------------------

/// Approved metadata links keyed by series name, then by provider.
///
/// A series can legitimately have several approved links (one primary plus
/// secondaries), so providers must be tracked as a set. Checking "does this
/// series have any link" would skip a provider that was never matched, which
/// is exactly the bug fixed in #150.
pub(super) type ApprovedLinks = HashMap<String, HashSet<String>>;

/// Group `(series_name, provider)` rows into a per-series provider set.
pub(super) fn approved_links_by_series(rows: Vec<(String, String)>) -> ApprovedLinks {
    let mut links: ApprovedLinks = HashMap::new();
    for (series_name, provider) in rows {
        links.entry(series_name).or_default().insert(provider);
    }
    links
}

/// Load every approved link of a library, grouped per series and provider.
pub(super) async fn load_approved_links(
    pool: &PgPool,
    library_id: Uuid,
) -> Result<ApprovedLinks, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT s.name, eml.provider FROM external_metadata_links eml \
         JOIN series s ON s.id = eml.series_id \
         WHERE eml.library_id = $1 AND eml.status = 'approved'",
    )
    .bind(library_id)
    .fetch_all(pool)
    .await?;

    Ok(approved_links_by_series(
        rows.into_iter()
            .map(|row| (row.get("name"), row.get("provider")))
            .collect(),
    ))
}

/// True when `provider` already has an approved link for `series_name`.
pub(super) fn has_approved_link(links: &ApprovedLinks, series_name: &str, provider: &str) -> bool {
    links
        .get(series_name)
        .is_some_and(|providers| providers.contains(provider))
}

// ---------------------------------------------------------------------------
// Search evaluation
// ---------------------------------------------------------------------------

pub(super) enum SearchOutcome {
    AutoMatch(metadata_providers::SeriesCandidate),
    NoResults,
    TooManyResults(i32, Option<metadata_providers::SeriesCandidate>),
    LowConfidence(metadata_providers::SeriesCandidate),
    Error(String),
}

/// The prudence rule shared by every provider: only a perfect confidence is
/// auto-approved. Anything below stays for manual review.
pub(super) fn is_confident_match(confidence: f32) -> bool {
    (confidence - 1.0).abs() < f32::EPSILON
}

/// Turn raw candidates into an outcome without touching the network or the DB.
pub(super) fn evaluate_candidates(
    candidates: Vec<metadata_providers::SeriesCandidate>,
) -> SearchOutcome {
    // Only auto-match if exactly 1 result with confidence == 1.0
    if candidates.len() == 1 && is_confident_match(candidates[0].confidence) {
        return SearchOutcome::AutoMatch(candidates.into_iter().next().unwrap());
    }

    // Check if best candidate has perfect confidence
    let Some(best) = candidates.into_iter().next() else {
        return SearchOutcome::NoResults;
    };
    if is_confident_match(best.confidence) {
        return SearchOutcome::AutoMatch(best);
    }

    if best.confidence < 1.0 {
        return SearchOutcome::LowConfidence(best);
    }

    SearchOutcome::TooManyResults(0, None)
}

pub(super) async fn search_and_evaluate(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    detailed: bool,
) -> SearchOutcome {
    // Canonicalize discovery source ids before lookup (#112) so a legacy name
    // does not produce a spurious "unknown provider" error. `resolve_provider`
    // reports the canonical name it rejected.
    let provider = match metadata_providers::resolve_provider(provider_name) {
        Ok(p) => p,
        Err(unknown) => return SearchOutcome::Error(format!("Unknown provider: {unknown}")),
    };
    let provider_name = provider.name();

    let mut config = super::config::load_provider_config(pool, provider_name).await;
    config.detailed = detailed;

    let candidates = match provider.search_series(series_name, &config).await {
        Ok(c) => c,
        Err(e) => return SearchOutcome::Error(e),
    };

    if candidates.is_empty() {
        return SearchOutcome::NoResults;
    }

    // Boost confidence based on local book count vs candidate total_volumes
    let local_count: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books b \
         JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 AND norm_text(s.name) = norm_text($2) \
         AND b.volume_type IN ('regular', 'integral', 'oneshot')",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_one(pool)
    .await
    .ok();

    let mut candidates = candidates;
    if let Some(count) = local_count {
        super::config::boost_confidence_by_book_count(&mut candidates, count);
    }

    evaluate_candidates(candidates)
}

// ---------------------------------------------------------------------------
// Per-provider outcome
// ---------------------------------------------------------------------------

/// Result of evaluating one provider for one series.
///
/// Keeping it provider-scoped lets the batch job evaluate the primary and the
/// secondary independently and report both in a single job event.
pub(super) struct ProviderOutcome {
    pub status: &'static str,
    pub provider: String,
    pub fallback_used: bool,
    pub candidates_count: i32,
    pub best_confidence: Option<f32>,
    pub best_candidate: Option<serde_json::Value>,
    pub link_id: Option<Uuid>,
    pub message: Option<String>,
}

impl ProviderOutcome {
    pub(super) fn is_auto_matched(&self) -> bool {
        self.status == "auto_matched"
    }

    /// Event `detail` payload for this provider (same shape as before #150,
    /// plus `message` so a nested secondary outcome keeps its explanation).
    pub(super) fn detail(&self) -> serde_json::Value {
        serde_json::json!({
            "status": self.status,
            "provider": self.provider,
            "fallback_used": self.fallback_used,
            "candidates_count": self.candidates_count,
            "confidence": self.best_confidence,
            "best_candidate": self.best_candidate,
            "link_id": self.link_id.map(|id| id.to_string()),
            "message": self.message,
        })
    }
}

/// Pick the outcome reported for a series, and whether it came from the
/// secondary provider.
///
/// A confident primary always wins. When the primary is not a confident match,
/// a confident secondary (fallback) is reported instead. An uncertain outcome
/// never wins, so it can never be turned into an auto-match here.
pub(super) fn select_main_outcome<'a>(
    primary: Option<&'a ProviderOutcome>,
    secondary: Option<&'a ProviderOutcome>,
) -> Option<(&'a ProviderOutcome, bool)> {
    let main_is_secondary = match (primary, secondary) {
        (Some(primary), Some(secondary)) => {
            !primary.is_auto_matched() && secondary.is_auto_matched()
        }
        (None, Some(_)) => true,
        _ => false,
    };

    let main = if main_is_secondary {
        secondary
    } else {
        primary.or(secondary)
    };

    main.map(|main| (main, main_is_secondary))
}

/// What the batch job should report for one series.
pub(super) enum SeriesOutcome<'a> {
    /// The series already has an approved link and nothing new was matched.
    AlreadyLinked,
    /// A provider lookup was skipped because SensCritique rate-limited the job.
    RateLimited,
    /// Report this provider outcome for the series.
    Reported {
        main: &'a ProviderOutcome,
        from_secondary: bool,
    },
}

/// Decide what to report for a series, taking existing links into account.
///
/// A series whose primary is already linked must not be downgraded to
/// `no_results` / `low_confidence` just because the secondary found nothing:
/// only a confident secondary changes what is reported. Conversely, a skipped
/// SensCritique lookup is only an error when the series has no link at all.
pub(super) fn select_series_outcome<'a>(
    primary_linked: bool,
    secondary_linked: bool,
    provider_skipped_rate_limited: bool,
    primary: Option<&'a ProviderOutcome>,
    secondary: Option<&'a ProviderOutcome>,
) -> SeriesOutcome<'a> {
    let secondary_matched = secondary.is_some_and(ProviderOutcome::is_auto_matched);

    if !(primary_linked && !secondary_matched) {
        if let Some((main, from_secondary)) = select_main_outcome(primary, secondary) {
            return SeriesOutcome::Reported {
                main,
                from_secondary,
            };
        }
    }

    if primary_linked || secondary_linked {
        SeriesOutcome::AlreadyLinked
    } else if provider_skipped_rate_limited {
        SeriesOutcome::RateLimited
    } else {
        // Neither provider produced an outcome and nothing is linked: nothing
        // to report, the series was already handled.
        SeriesOutcome::AlreadyLinked
    }
}

/// Candidate payload persisted in job events (used by the quick-match UI).
fn candidate_detail(
    candidate: &metadata_providers::SeriesCandidate,
    include_details: bool,
) -> serde_json::Value {
    if include_details {
        serde_json::json!({
            "title": candidate.title,
            "external_id": candidate.external_id,
            "external_url": candidate.external_url,
            "authors": candidate.authors,
            "description": candidate.description,
            "cover_url": candidate.cover_url,
            "total_volumes": candidate.total_volumes,
            "start_year": candidate.start_year,
            "confidence": candidate.confidence,
        })
    } else {
        serde_json::json!({
            "title": candidate.title,
            "external_id": candidate.external_id,
        })
    }
}

/// Search one provider for one series and auto-apply a confident match.
///
/// The low-confidence / too-many-results outcomes never create a link: they are
/// reported for manual review, exactly like the primary provider.
pub(super) async fn evaluate_provider(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    fallback_used: bool,
) -> ProviderOutcome {
    let outcome = search_and_evaluate(pool, library_id, series_name, provider_name, true).await;

    match outcome {
        SearchOutcome::AutoMatch(candidate) => {
            match auto_apply(pool, library_id, series_name, provider_name, &candidate).await {
                Ok(link_id) => ProviderOutcome {
                    status: "auto_matched",
                    provider: provider_name.to_string(),
                    fallback_used,
                    candidates_count: 1,
                    best_confidence: Some(candidate.confidence),
                    best_candidate: Some(candidate_detail(&candidate, false)),
                    link_id: Some(link_id),
                    message: None,
                },
                Err(e) => ProviderOutcome {
                    status: "error",
                    provider: provider_name.to_string(),
                    fallback_used,
                    candidates_count: 1,
                    best_confidence: Some(candidate.confidence),
                    best_candidate: None,
                    link_id: None,
                    message: Some(format!("Auto-apply failed: {e}")),
                },
            }
        }
        SearchOutcome::NoResults => ProviderOutcome {
            status: "no_results",
            provider: provider_name.to_string(),
            fallback_used,
            candidates_count: 0,
            best_confidence: None,
            best_candidate: None,
            link_id: None,
            message: Some("No results found".to_string()),
        },
        SearchOutcome::TooManyResults(count, best) => ProviderOutcome {
            status: "too_many_results",
            provider: provider_name.to_string(),
            fallback_used,
            candidates_count: count,
            best_confidence: best.as_ref().map(|c| c.confidence),
            best_candidate: best.as_ref().map(|c| candidate_detail(c, true)),
            link_id: None,
            message: Some(format!("{count} results, manual review needed")),
        },
        SearchOutcome::LowConfidence(candidate) => ProviderOutcome {
            status: "low_confidence",
            provider: provider_name.to_string(),
            fallback_used,
            candidates_count: 1,
            best_confidence: Some(candidate.confidence),
            best_candidate: Some(candidate_detail(&candidate, true)),
            link_id: None,
            message: Some(format!(
                "Best confidence: {:.0}%",
                candidate.confidence * 100.0
            )),
        },
        SearchOutcome::Error(e) => ProviderOutcome {
            status: "error",
            provider: provider_name.to_string(),
            fallback_used,
            candidates_count: 0,
            best_confidence: None,
            best_candidate: None,
            link_id: None,
            message: Some(e),
        },
    }
}

pub(super) async fn auto_apply(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<Uuid, String> {
    // Resolve series_id from series name
    let series_id: Uuid =
        sqlx::query_scalar("SELECT id FROM series WHERE library_id = $1 AND name = $2")
            .bind(library_id)
            .bind(series_name)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Series '{}' not found in library", series_name))?;

    // Create the external_metadata_link
    let metadata_json = &candidate.metadata_json;
    let (pr_rating, pr_rating_count, pr_rating_scale) =
        shared_sync::extract_provider_rating(metadata_json);
    let row = sqlx::query(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external,
             provider_rating, provider_rating_count, provider_rating_scale)
        VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8, $9, $10, $11)
        ON CONFLICT (series_id, provider)
        DO UPDATE SET
            external_id = EXCLUDED.external_id,
            external_url = EXCLUDED.external_url,
            status = 'approved',
            confidence = EXCLUDED.confidence,
            metadata_json = EXCLUDED.metadata_json,
            total_volumes_external = EXCLUDED.total_volumes_external,
            provider_rating = EXCLUDED.provider_rating,
            provider_rating_count = EXCLUDED.provider_rating_count,
            provider_rating_scale = EXCLUDED.provider_rating_scale,
            matched_at = NOW(),
            approved_at = NOW(),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .bind(provider_name)
    .bind(&candidate.external_id)
    .bind(&candidate.external_url)
    .bind(candidate.confidence)
    .bind(metadata_json)
    .bind(candidate.total_volumes)
    .bind(pr_rating)
    .bind(pr_rating_count)
    .bind(pr_rating_scale)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let link_id: Uuid = row.get("id");

    // The first approved link of a series becomes the primary.
    shared_sync::promote_if_no_primary(pool, series_id, link_id)
        .await
        .map_err(|e| e.to_string())?;

    // Sync the whole series from all approved links (primary first, secondaries
    // as fallback), so an auto-matched secondary never overwrites the primary.
    super::sync_series_from_links(pool, series_id, true, true)
        .await
        .map_err(|e| e.message)?;

    Ok(link_id)
}

/// Rematch cleanup: drop links from providers that are no longer configured.
///
/// The configured providers are kept on purpose: now that one job can match
/// both the primary and the secondary, deleting the freshly created secondary
/// link would undo half of the job (#150). Removing the old primary would also
/// leave the series without a reference provider, so the oldest remaining
/// approved link is promoted afterwards.
pub(super) async fn cleanup_rematch_links(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    keep_providers: &[String],
) -> Result<u64, sqlx::Error> {
    let series_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM series WHERE library_id = $1 AND name = $2")
            .bind(library_id)
            .bind(series_name)
            .fetch_optional(pool)
            .await?;

    let Some(series_id) = series_id else {
        return Ok(0);
    };

    let result = sqlx::query(
        "DELETE FROM external_metadata_links \
         WHERE library_id = $1 AND series_id = $2 AND provider <> ALL($3)",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(keep_providers)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        shared_sync::promote_oldest_approved(pool, series_id).await?;
    }

    Ok(result.rows_affected())
}

#[cfg(test)]
#[path = "tests/batch_sync.rs"]
mod tests;

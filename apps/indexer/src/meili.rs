use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

#[derive(Serialize)]
struct SearchDoc {
    id: String,
    library_id: String,
    kind: String,
    title: String,
    author: Option<String>,
    series: Option<String>,
    volume: Option<i32>,
    language: Option<String>,
}

pub async fn sync_meili(pool: &PgPool, meili_url: &str, meili_master_key: &str) -> Result<()> {
    let client = Client::new();
    let base = meili_url.trim_end_matches('/');

    // Ensure index exists and has proper settings
    let _ = client
        .post(format!("{base}/indexes"))
        .header("Authorization", format!("Bearer {meili_master_key}"))
        .json(&serde_json::json!({"uid": "books", "primaryKey": "id"}))
        .send()
        .await;

    let _ = client
        .patch(format!("{base}/indexes/books/settings/filterable-attributes"))
        .header("Authorization", format!("Bearer {meili_master_key}"))
        .json(&serde_json::json!(["library_id", "kind"]))
        .send()
        .await;

    // Get last sync timestamp
    let last_sync: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT last_meili_sync FROM sync_metadata WHERE id = 1 AND last_meili_sync IS NOT NULL"
    )
    .fetch_optional(pool)
    .await?;

    // If no previous sync, do a full sync
    let is_full_sync = last_sync.is_none();
    
    // Get books to sync: all if full sync, only modified since last sync otherwise
    let rows = if is_full_sync {
        info!("[MEILI] Performing full sync");
        sqlx::query(
            "SELECT id, library_id, kind, title, author, series, volume, language, updated_at FROM books",
        )
        .fetch_all(pool)
        .await?
    } else {
        let since = last_sync.unwrap();
        info!("[MEILI] Performing incremental sync since {}", since);
        
        // Also get deleted book IDs to remove from MeiliSearch
        // For now, we'll do a diff approach: get all book IDs from DB and compare with Meili
        sqlx::query(
            "SELECT id, library_id, kind, title, author, series, volume, language, updated_at FROM books WHERE updated_at > $1",
        )
        .bind(since)
        .fetch_all(pool)
        .await?
    };

    if rows.is_empty() && !is_full_sync {
        info!("[MEILI] No changes to sync");
        // Still update the timestamp
        sqlx::query(
            "INSERT INTO sync_metadata (id, last_meili_sync) VALUES (1, NOW()) ON CONFLICT (id) DO UPDATE SET last_meili_sync = NOW()"
        )
        .execute(pool)
        .await?;
        return Ok(());
    }

    let docs: Vec<SearchDoc> = rows
        .into_iter()
        .map(|row| SearchDoc {
            id: row.get::<Uuid, _>("id").to_string(),
            library_id: row.get::<Uuid, _>("library_id").to_string(),
            kind: row.get("kind"),
            title: row.get("title"),
            author: row.get("author"),
            series: row.get("series"),
            volume: row.get("volume"),
            language: row.get("language"),
        })
        .collect();

    let doc_count = docs.len();
    
    // Send documents to MeiliSearch in batches of 1000
    const MEILI_BATCH_SIZE: usize = 1000;
    for (i, chunk) in docs.chunks(MEILI_BATCH_SIZE).enumerate() {
        let batch_num = i + 1;
        info!("[MEILI] Sending batch {}/{} ({} docs)", batch_num, doc_count.div_ceil(MEILI_BATCH_SIZE), chunk.len());
        
        let response = client
            .post(format!("{base}/indexes/books/documents"))
            .header("Authorization", format!("Bearer {meili_master_key}"))
            .json(&chunk)
            .send()
            .await
            .context("failed to send docs to meili")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("MeiliSearch error {}: {}", status, body));
        }
    }

    // Clean up stale documents: remove from Meilisearch any IDs that no longer exist in DB.
    // Runs on every sync — the cost is minimal (single fetch of IDs only).
    {
        let db_ids: Vec<String> = sqlx::query_scalar("SELECT id::text FROM books")
            .fetch_all(pool)
            .await?;

        // Fetch all document IDs from Meilisearch (paginated to handle large collections)
        let mut meili_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut offset: usize = 0;
        const PAGE_SIZE: usize = 10000;

        loop {
            let response = client
                .post(format!("{base}/indexes/books/documents/fetch"))
                .header("Authorization", format!("Bearer {meili_master_key}"))
                .json(&serde_json::json!({
                    "fields": ["id"],
                    "limit": PAGE_SIZE,
                    "offset": offset
                }))
                .send()
                .await;

            let response = match response {
                Ok(r) if r.status().is_success() => r,
                _ => break,
            };

            let payload: serde_json::Value = match response.json().await {
                Ok(v) => v,
                Err(_) => break,
            };

            let results = payload.get("results")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let page_count = results.len();
            for doc in results {
                if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
                    meili_ids.insert(id.to_string());
                }
            }

            if page_count < PAGE_SIZE {
                break; // Last page
            }
            offset += PAGE_SIZE;
        }

        let db_ids_set: std::collections::HashSet<String> = db_ids.into_iter().collect();
        let to_delete: Vec<String> = meili_ids.difference(&db_ids_set).cloned().collect();

        if !to_delete.is_empty() {
            info!("[MEILI] Deleting {} stale documents", to_delete.len());
            let _ = client
                .post(format!("{base}/indexes/books/documents/delete-batch"))
                .header("Authorization", format!("Bearer {meili_master_key}"))
                .json(&to_delete)
                .send()
                .await;
        }
    }

    // Update last sync timestamp
    sqlx::query(
        "INSERT INTO sync_metadata (id, last_meili_sync) VALUES (1, NOW()) ON CONFLICT (id) DO UPDATE SET last_meili_sync = NOW()"
    )
    .execute(pool)
    .await?;

    info!("[MEILI] Sync completed: {} documents indexed", doc_count);
    Ok(())
}

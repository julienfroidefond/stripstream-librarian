use anyhow::Result;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_events")]
    pub events: EventToggles,
}

#[derive(Debug, Deserialize)]
pub struct EventToggles {
    #[serde(default = "default_true")]
    pub scan_completed: bool,
    #[serde(default = "default_true")]
    pub scan_failed: bool,
    #[serde(default = "default_true")]
    pub scan_cancelled: bool,
    #[serde(default = "default_true")]
    pub thumbnail_completed: bool,
    #[serde(default = "default_true")]
    pub thumbnail_failed: bool,
    #[serde(default = "default_true")]
    pub conversion_completed: bool,
    #[serde(default = "default_true")]
    pub conversion_failed: bool,
    #[serde(default = "default_true")]
    pub metadata_approved: bool,
    #[serde(default = "default_true")]
    pub metadata_batch_completed: bool,
    #[serde(default = "default_true")]
    pub metadata_batch_failed: bool,
    #[serde(default = "default_true")]
    pub metadata_refresh_completed: bool,
    #[serde(default = "default_true")]
    pub metadata_refresh_failed: bool,
    #[serde(default = "default_true")]
    pub reading_status_match_completed: bool,
    #[serde(default = "default_true")]
    pub reading_status_match_failed: bool,
    #[serde(default = "default_true")]
    pub reading_status_push_completed: bool,
    #[serde(default = "default_true")]
    pub reading_status_push_failed: bool,
    #[serde(default = "default_true")]
    pub download_detection_completed: bool,
    #[serde(default = "default_true")]
    pub download_detection_failed: bool,
    #[serde(default = "default_true")]
    pub torrent_import_completed: bool,
    #[serde(default = "default_true")]
    pub torrent_import_failed: bool,
    #[serde(default = "default_true")]
    pub telegram_sync_incremental_completed: bool,
}

fn default_true() -> bool {
    true
}

fn default_events() -> EventToggles {
    EventToggles {
        scan_completed: true,
        scan_failed: true,
        scan_cancelled: true,
        thumbnail_completed: true,
        thumbnail_failed: true,
        conversion_completed: true,
        conversion_failed: true,
        metadata_approved: true,
        metadata_batch_completed: true,
        metadata_batch_failed: true,
        metadata_refresh_completed: true,
        metadata_refresh_failed: true,
        reading_status_match_completed: true,
        reading_status_match_failed: true,
        reading_status_push_completed: true,
        reading_status_push_failed: true,
        download_detection_completed: true,
        download_detection_failed: true,
        torrent_import_completed: true,
        torrent_import_failed: true,
        telegram_sync_incremental_completed: true,
    }
}

/// Load the Telegram config from `app_settings` (key = "telegram").
/// Returns `None` when the row is missing, disabled, or has empty credentials.
pub async fn load_telegram_config(pool: &PgPool) -> Option<TelegramConfig> {
    let row = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_settings WHERE key = 'telegram'",
    )
    .fetch_optional(pool)
    .await
    .ok()??;

    let config: TelegramConfig = serde_json::from_value(row).ok()?;

    if !config.enabled || config.bot_token.is_empty() || config.chat_id.is_empty() {
        return None;
    }

    Some(config)
}

// ---------------------------------------------------------------------------
// Telegram HTTP
// ---------------------------------------------------------------------------

fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?)
}

async fn send_telegram(config: &TelegramConfig, text: &str) -> Result<()> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.bot_token
    );

    let body = serde_json::json!({
        "chat_id": config.chat_id,
        "text": text,
        "parse_mode": "HTML",
    });

    let resp = build_client()?.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram API returned {status}: {text}");
    }

    Ok(())
}

async fn send_telegram_photo(config: &TelegramConfig, caption: &str, photo_path: &str) -> Result<()> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendPhoto",
        config.bot_token
    );

    let photo_bytes = tokio::fs::read(photo_path).await?;
    let filename = std::path::Path::new(photo_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mime = if filename.ends_with(".webp") {
        "image/webp"
    } else if filename.ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };

    let part = reqwest::multipart::Part::bytes(photo_bytes)
        .file_name(filename)
        .mime_str(mime)?;

    let form = reqwest::multipart::Form::new()
        .text("chat_id", config.chat_id.clone())
        .text("caption", caption.to_string())
        .text("parse_mode", "HTML")
        .part("photo", part);

    let resp = build_client()?.post(&url).multipart(form).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram API returned {status}: {text}");
    }

    Ok(())
}

/// Send a test message. Returns the result directly (not fire-and-forget).
pub async fn send_test_message(config: &TelegramConfig) -> Result<()> {
    send_telegram(
        config,
        "🔔 <b>Stripstream Librarian</b>\n\
         ✅ Test notification — connection OK!",
    )
    .await
}

// ---------------------------------------------------------------------------
// Notification events
// ---------------------------------------------------------------------------

pub struct ScanStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub removed_files: usize,
    pub new_series: usize,
    pub errors: usize,
}

pub enum NotificationEvent {
    // Scan jobs (rebuild, full_rebuild, rescan, scan)
    ScanCompleted {
        job_type: String,
        library_name: Option<String>,
        stats: ScanStats,
        duration_seconds: u64,
    },
    ScanFailed {
        job_type: String,
        library_name: Option<String>,
        error: String,
    },
    ScanCancelled {
        job_type: String,
        library_name: Option<String>,
    },
    // Thumbnail jobs (thumbnail_rebuild, thumbnail_regenerate)
    ThumbnailCompleted {
        job_type: String,
        library_name: Option<String>,
        duration_seconds: u64,
    },
    ThumbnailFailed {
        job_type: String,
        library_name: Option<String>,
        error: String,
    },
    // CBR→CBZ conversion
    ConversionCompleted {
        library_name: Option<String>,
        book_title: Option<String>,
        thumbnail_path: Option<String>,
    },
    ConversionFailed {
        library_name: Option<String>,
        book_title: Option<String>,
        thumbnail_path: Option<String>,
        error: String,
    },
    // Metadata manual approve
    MetadataApproved {
        series_name: String,
        provider: String,
        thumbnail_path: Option<String>,
        fields_updated: Vec<String>,
        books_matched: usize,
        books_updated: usize,
    },
    // Metadata batch (auto-match)
    MetadataBatchCompleted {
        library_name: Option<String>,
        total_series: i32,
        processed: i32,
        auto_matched: i64,
        no_results: i64,
        low_confidence: i64,
        too_many: i64,
        already_linked: i64,
        errors: i64,
        /// Series newly auto-matched this run, capped at 10
        new_matches: Vec<String>,
    },
    MetadataBatchFailed {
        library_name: Option<String>,
        error: String,
    },
    // Metadata refresh
    MetadataRefreshCompleted {
        library_name: Option<String>,
        refreshed: i32,
        unchanged: i32,
        errors: i32,
        series_fields_updated: usize,
        books_fields_updated: usize,
        details: Vec<String>,
    },
    MetadataRefreshFailed {
        library_name: Option<String>,
        error: String,
    },
    // Reading status match (auto-link series to provider)
    ReadingStatusMatchCompleted {
        library_name: Option<String>,
        total_series: i32,
        linked: i32,
        already_linked: i64,
        no_results: i64,
        ambiguous: i64,
        errors: i64,
    },
    ReadingStatusMatchFailed {
        library_name: Option<String>,
        error: String,
    },
    // Reading status push (differential push to AniList)
    ReadingStatusPushCompleted {
        library_name: Option<String>,
        total_series: i32,
        pushed: i32,
        skipped: i64,
        no_books: i64,
        errors: i64,
    },
    ReadingStatusPushFailed {
        library_name: Option<String>,
        error: String,
    },
    // Download detection (Prowlarr search for missing volumes)
    DownloadDetectionCompleted {
        library_name: Option<String>,
        total_series: i32,
        found: i64,
        new_releases: i64,
        not_found: i64,
        no_missing: i64,
        no_metadata: i64,
        errors: i64,
        /// New releases detected this run: (series_name, release_title), capped at 10
        new_items: Vec<(String, String)>,
    },
    DownloadDetectionFailed {
        library_name: Option<String>,
        error: String,
    },
    // Torrent import (qBittorrent download completed → files imported into library)
    TorrentImportCompleted {
        library_name: Option<String>,
        series_name: String,
        imported_count: usize,
        volumes: Vec<i32>,
    },
    TorrentImportFailed {
        library_name: Option<String>,
        series_name: String,
        error: String,
    },
    // Telegram incremental sync completed
    TelegramSyncIncrementalCompleted {
        new_books: usize,
        sources_scanned: usize,
        /// Per-source breakdown: (channel_username, new_books_count)
        per_source: Vec<(String, usize)>,
        /// New books detected: (series_name_or_filename, volume_number), capped at 15
        new_items: Vec<(String, Option<i32>)>,
    },
}

/// Classify an indexer job_type string into the right event constructor category.
/// Returns "scan", "thumbnail", or "conversion".
pub fn job_type_category(job_type: &str) -> &'static str {
    match job_type {
        "thumbnail_rebuild" | "thumbnail_regenerate" => "thumbnail",
        "cbr_to_cbz" => "conversion",
        _ => "scan",
    }
}

fn format_event(event: &NotificationEvent) -> String {
    match event {
        NotificationEvent::ScanCompleted {
            job_type,
            library_name,
            stats,
            duration_seconds,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let duration = format_duration(*duration_seconds);
            let mut lines = vec![
                format!("✅ <b>Scan completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("🏷 <b>Type:</b> {job_type}"),
                format!("⏱ <b>Duration:</b> {duration}"),
                String::new(),
                format!("📊 <b>Results</b>"),
                format!("  📗 New books: <b>{}</b>", stats.indexed_files),
                format!("  📚 New series: <b>{}</b>", stats.new_series),
                format!("  🔎 Files scanned: <b>{}</b>", stats.scanned_files),
                format!("  🗑 Removed: <b>{}</b>", stats.removed_files),
            ];
            if stats.errors > 0 {
                lines.push(format!("  ⚠️ Errors: <b>{}</b>", stats.errors));
            }
            lines.join("\n")
        }
        NotificationEvent::ScanFailed {
            job_type,
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Scan failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("🏷 <b>Type:</b> {job_type}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::ScanCancelled {
            job_type,
            library_name,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            [
                "⏹ <b>Scan cancelled</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("🏷 <b>Type:</b> {job_type}"),
            ]
            .join("\n")
        }
        NotificationEvent::ThumbnailCompleted {
            job_type,
            library_name,
            duration_seconds,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let duration = format_duration(*duration_seconds);
            [
                "✅ <b>Thumbnails completed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("🏷 <b>Type:</b> {job_type}"),
                format!("⏱ <b>Duration:</b> {duration}"),
            ]
            .join("\n")
        }
        NotificationEvent::ThumbnailFailed {
            job_type,
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Thumbnails failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("🏷 <b>Type:</b> {job_type}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::ConversionCompleted {
            library_name,
            book_title,
            ..
        } => {
            let lib = library_name.as_deref().unwrap_or("Unknown");
            let title = book_title.as_deref().unwrap_or("Unknown");
            [
                "✅ <b>CBR → CBZ conversion completed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📖 <b>Book:</b> {title}"),
            ]
            .join("\n")
        }
        NotificationEvent::ConversionFailed {
            library_name,
            book_title,
            error,
            ..
        } => {
            let lib = library_name.as_deref().unwrap_or("Unknown");
            let title = book_title.as_deref().unwrap_or("Unknown");
            let err = truncate(error, 200);
            [
                "🚨 <b>CBR → CBZ conversion failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📖 <b>Book:</b> {title}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::MetadataApproved {
            series_name,
            provider,
            fields_updated,
            books_matched,
            books_updated,
            ..
        } => {
            let mut lines = vec![
                format!("✅ <b>Metadata linked</b>"),
                String::new(),
                format!("📚 <b>Series:</b> {series_name}"),
                format!("🔗 <b>Provider:</b> {provider}"),
            ];
            if !fields_updated.is_empty() {
                lines.push(format!("📝 <b>Fields:</b> {}", fields_updated.join(", ")));
            }
            if *books_matched > 0 || *books_updated > 0 {
                lines.push(format!("📖 <b>Books:</b> {} matched, {} updated", books_matched, books_updated));
            }
            lines.join("\n")
        }
        NotificationEvent::MetadataBatchCompleted {
            library_name,
            total_series,
            processed,
            auto_matched,
            no_results,
            low_confidence,
            too_many,
            already_linked,
            errors,
            new_matches,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let mut lines = vec![
                format!("📊 <b>Metadata batch completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📋 <b>Processed:</b> {processed}/{total_series} series"),
            ];
            if *auto_matched > 0 {
                lines.push(format!("✅ Auto-matched: <b>{auto_matched}</b>"));
            }
            if !new_matches.is_empty() {
                lines.push(String::new());
                lines.push("🆕 <b>New matches:</b>".to_string());
                for name in new_matches.iter().take(10) {
                    lines.push(format!("  • {}", truncate(name, 60)));
                }
                if new_matches.len() > 10 {
                    lines.push(format!("  … and {} more", new_matches.len() - 10));
                }
            }
            let mut warnings = Vec::new();
            if *low_confidence > 0 {
                warnings.push(format!("{low_confidence} low confidence"));
            }
            if *too_many > 0 {
                warnings.push(format!("{too_many} too many results"));
            }
            if !warnings.is_empty() {
                lines.push(format!("⚠️ {}", warnings.join(", ")));
            }
            if *errors > 0 {
                lines.push(format!("❌ Errors: <b>{errors}</b>"));
            }
            let mut skips = Vec::new();
            if *already_linked > 0 {
                skips.push(format!("{already_linked} already linked"));
            }
            if *no_results > 0 {
                skips.push(format!("{no_results} no results"));
            }
            if !skips.is_empty() {
                lines.push(format!("⏭️ {}", skips.join(", ")));
            }
            lines.join("\n")
        }
        NotificationEvent::MetadataBatchFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Metadata batch failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::MetadataRefreshCompleted {
            library_name,
            refreshed,
            unchanged,
            errors,
            series_fields_updated,
            books_fields_updated,
            details,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let mut lines = vec![
                format!("🔄 <b>Metadata refresh completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📝 {refreshed} series refreshed, {unchanged} unchanged"),
            ];
            if *series_fields_updated > 0 || *books_fields_updated > 0 {
                lines.push(format!("📊 {series_fields_updated} series fields, {books_fields_updated} book fields updated"));
            }
            if *errors > 0 {
                lines.push(format!("❌ Errors: <b>{errors}</b>"));
            }
            for detail in details.iter().take(5) {
                lines.push(format!("📖 {detail}"));
            }
            lines.join("\n")
        }
        NotificationEvent::MetadataRefreshFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Metadata refresh failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::ReadingStatusMatchCompleted {
            library_name,
            total_series,
            linked,
            already_linked,
            no_results,
            ambiguous,
            errors,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let mut lines = vec![
                format!("✅ <b>Reading status match completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("🔗 Linked: <b>{linked}</b> / <b>{total_series}</b> series"),
            ];
            if *already_linked > 0 {
                lines.push(format!("⏭️ Already linked: <b>{already_linked}</b>"));
            }
            if *ambiguous > 0 {
                lines.push(format!("⚠️ Ambiguous: <b>{ambiguous}</b>"));
            }
            if *no_results > 0 {
                lines.push(format!("🔍 No results: <b>{no_results}</b>"));
            }
            if *errors > 0 {
                lines.push(format!("❌ Errors: <b>{errors}</b>"));
            }
            lines.join("\n")
        }
        NotificationEvent::ReadingStatusMatchFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Reading status match failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::ReadingStatusPushCompleted {
            library_name,
            total_series,
            pushed,
            skipped,
            no_books,
            errors,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let mut lines = vec![
                format!("✅ <b>Reading status push completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("⬆️ Pushed: <b>{pushed}</b> / <b>{total_series}</b> series"),
            ];
            if *skipped > 0 {
                lines.push(format!("⏭️ Skipped: <b>{skipped}</b>"));
            }
            if *no_books > 0 {
                lines.push(format!("📭 No books: <b>{no_books}</b>"));
            }
            if *errors > 0 {
                lines.push(format!("❌ Errors: <b>{errors}</b>"));
            }
            lines.join("\n")
        }
        NotificationEvent::ReadingStatusPushFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Reading status push failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::DownloadDetectionCompleted {
            library_name,
            total_series,
            found,
            new_releases,
            not_found,
            no_missing,
            no_metadata,
            errors,
            new_items,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let mut lines = vec![
                format!("✅ <b>Download detection completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📥 Found: <b>{found}</b> / <b>{total_series}</b> series"),
            ];
            if *new_releases > 0 {
                lines.push(format!("🆕 New releases: <b>{new_releases}</b>"));
            }
            if !new_items.is_empty() {
                lines.push(String::new());
                lines.push("📦 <b>New releases detected:</b>".to_string());
                for (series, title) in new_items.iter().take(10) {
                    lines.push(format!("  • <b>{}</b> — {}", truncate(series, 40), truncate(title, 60)));
                }
                if new_items.len() > 10 {
                    lines.push(format!("  … and {} more", new_items.len() - 10));
                }
            }
            if *not_found > 0 {
                lines.push(format!("🔍 Not found: <b>{not_found}</b>"));
            }
            if *no_missing > 0 {
                lines.push(format!("⏭️ No missing volumes: <b>{no_missing}</b>"));
            }
            if *no_metadata > 0 {
                lines.push(format!("📭 No metadata: <b>{no_metadata}</b>"));
            }
            if *errors > 0 {
                lines.push(format!("❌ Errors: <b>{errors}</b>"));
            }
            lines.join("\n")
        }
        NotificationEvent::DownloadDetectionFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                "🚨 <b>Download detection failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::TorrentImportCompleted {
            library_name,
            series_name,
            imported_count,
            volumes,
        } => {
            let lib = library_name.as_deref().unwrap_or("Unknown");
            let mut lines = vec![
                format!("📥 <b>Torrent import completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📚 <b>{series_name}</b> — {imported_count} files imported"),
            ];
            if !volumes.is_empty() {
                let mut sorted = volumes.clone();
                sorted.sort();
                let vol_list: Vec<String> = sorted.iter().map(|v| v.to_string()).collect();
                lines.push(format!("📖 Volumes: {}", vol_list.join(", ")));
            }
            lines.join("\n")
        }
        NotificationEvent::TorrentImportFailed {
            library_name,
            series_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("Unknown");
            let err = truncate(error, 200);
            [
                "🚨 <b>Torrent import failed</b>".to_string(),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📚 <b>Series:</b> {series_name}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
        }
        NotificationEvent::TelegramSyncIncrementalCompleted {
            new_books,
            sources_scanned,
            per_source,
            new_items,
        } => {
            let mut lines = vec![
                "📡 <b>Telegram — nouveaux livres détectés</b>".to_string(),
                String::new(),
                format!("📥 <b>{new_books}</b> nouveau{} livre{} sur <b>{sources_scanned}</b> channel{}",
                    if *new_books > 1 { "x" } else { "" },
                    if *new_books > 1 { "s" } else { "" },
                    if *sources_scanned > 1 { "s" } else { "" },
                ),
            ];
            let active_sources: Vec<_> = per_source.iter().filter(|(_, c)| *c > 0).collect();
            if active_sources.len() > 1 {
                lines.push(String::new());
                for (username, count) in &active_sources {
                    lines.push(format!("  • @{username}: <b>{count}</b>"));
                }
            }
            if !new_items.is_empty() {
                lines.push(String::new());
                lines.push("📦 <b>Nouveaux livres :</b>".to_string());
                for (label, vol) in new_items.iter().take(15) {
                    let vol_str = vol.map(|v| format!(" T{v:02}")).unwrap_or_default();
                    lines.push(format!("  • <b>{}</b>{}", truncate(label, 50), vol_str));
                }
                if new_items.len() > 15 {
                    lines.push(format!("  … et {} de plus", new_items.len() - 15));
                }
            }
            lines.join("\n")
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s.to_string()
    }
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        let m = secs / 60;
        let s = secs % 60;
        format!("{m}m{s}s")
    }
}

// ---------------------------------------------------------------------------
// Public entry point — fire & forget
// ---------------------------------------------------------------------------

/// Returns whether this event type is enabled in the config.
fn is_event_enabled(config: &TelegramConfig, event: &NotificationEvent) -> bool {
    match event {
        NotificationEvent::ScanCompleted { .. } => config.events.scan_completed,
        NotificationEvent::ScanFailed { .. } => config.events.scan_failed,
        NotificationEvent::ScanCancelled { .. } => config.events.scan_cancelled,
        NotificationEvent::ThumbnailCompleted { .. } => config.events.thumbnail_completed,
        NotificationEvent::ThumbnailFailed { .. } => config.events.thumbnail_failed,
        NotificationEvent::ConversionCompleted { .. } => config.events.conversion_completed,
        NotificationEvent::ConversionFailed { .. } => config.events.conversion_failed,
        NotificationEvent::MetadataApproved { .. } => config.events.metadata_approved,
        NotificationEvent::MetadataBatchCompleted { .. } => config.events.metadata_batch_completed,
        NotificationEvent::MetadataBatchFailed { .. } => config.events.metadata_batch_failed,
        NotificationEvent::MetadataRefreshCompleted { .. } => config.events.metadata_refresh_completed,
        NotificationEvent::MetadataRefreshFailed { .. } => config.events.metadata_refresh_failed,
        NotificationEvent::ReadingStatusMatchCompleted { .. } => config.events.reading_status_match_completed,
        NotificationEvent::ReadingStatusMatchFailed { .. } => config.events.reading_status_match_failed,
        NotificationEvent::ReadingStatusPushCompleted { .. } => config.events.reading_status_push_completed,
        NotificationEvent::ReadingStatusPushFailed { .. } => config.events.reading_status_push_failed,
        NotificationEvent::DownloadDetectionCompleted { .. } => config.events.download_detection_completed,
        NotificationEvent::DownloadDetectionFailed { .. } => config.events.download_detection_failed,
        NotificationEvent::TorrentImportCompleted { .. } => config.events.torrent_import_completed,
        NotificationEvent::TorrentImportFailed { .. } => config.events.torrent_import_failed,
        NotificationEvent::TelegramSyncIncrementalCompleted { .. } => config.events.telegram_sync_incremental_completed,
    }
}

/// Returns whether this event carries meaningful information worth notifying about.
/// Filters out "empty" successes (e.g. a scan that found nothing new).
fn is_noteworthy(event: &NotificationEvent) -> bool {
    match event {
        // Scan: only notify if something changed (new books, removals, new series, errors)
        NotificationEvent::ScanCompleted { stats, .. } => {
            stats.indexed_files > 0
                || stats.removed_files > 0
                || stats.new_series > 0
                || stats.errors > 0
        }
        // Cancelled by user — they already know
        NotificationEvent::ScanCancelled { .. } => false,
        // Metadata batch: only if something was actually matched
        NotificationEvent::MetadataBatchCompleted { processed, .. } => *processed > 0,
        // Metadata refresh: only if something changed or errored
        NotificationEvent::MetadataRefreshCompleted {
            refreshed, errors, ..
        } => *refreshed > 0 || *errors > 0,
        // Reading status match: only if new links were made
        NotificationEvent::ReadingStatusMatchCompleted { linked, .. } => *linked > 0,
        // Reading status push: only if something was pushed
        NotificationEvent::ReadingStatusPushCompleted { pushed, .. } => *pushed > 0,
        // Download detection: only if releases were found
        NotificationEvent::DownloadDetectionCompleted { found, .. } => *found > 0,
        // Telegram incremental: only if new books were found
        NotificationEvent::TelegramSyncIncrementalCompleted { new_books, .. } => *new_books > 0,
        // All failures, conversions, imports, manual actions → always noteworthy
        _ => true,
    }
}

/// Extract thumbnail path from event if present and file exists on disk.
fn event_thumbnail(event: &NotificationEvent) -> Option<&str> {
    let path = match event {
        NotificationEvent::ConversionCompleted { thumbnail_path, .. } => thumbnail_path.as_deref(),
        NotificationEvent::ConversionFailed { thumbnail_path, .. } => thumbnail_path.as_deref(),
        NotificationEvent::MetadataApproved { thumbnail_path, .. } => thumbnail_path.as_deref(),
        _ => None,
    };
    path.filter(|p| std::path::Path::new(p).exists())
}

/// Load config + format + send in a spawned task. Errors are only logged.
pub fn notify(pool: PgPool, event: NotificationEvent) {
    tokio::spawn(async move {
        let config = match load_telegram_config(&pool).await {
            Some(c) => c,
            None => return, // disabled or not configured
        };

        if !is_event_enabled(&config, &event) {
            return;
        }

        if !is_noteworthy(&event) {
            info!("[TELEGRAM] Skipping non-noteworthy event");
            return;
        }

        let text = format_event(&event);
        let sent = if let Some(photo) = event_thumbnail(&event) {
            match send_telegram_photo(&config, &text, photo).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    warn!("[TELEGRAM] Photo send failed, falling back to text: {e}");
                    send_telegram(&config, &text).await
                }
            }
        } else {
            send_telegram(&config, &text).await
        };

        match sent {
            Ok(()) => info!("[TELEGRAM] Notification sent"),
            Err(e) => warn!("[TELEGRAM] Failed to send notification: {e}"),
        }
    });
}

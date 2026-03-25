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
    },
    // Metadata batch (auto-match)
    MetadataBatchCompleted {
        library_name: Option<String>,
        total_series: i32,
        processed: i32,
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
    },
    ReadingStatusPushFailed {
        library_name: Option<String>,
        error: String,
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
                format!("🚨 <b>Scan failed</b>"),
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
                format!("⏹ <b>Scan cancelled</b>"),
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
                format!("✅ <b>Thumbnails completed</b>"),
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
                format!("🚨 <b>Thumbnails failed</b>"),
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
                format!("✅ <b>CBR → CBZ conversion completed</b>"),
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
                format!("🚨 <b>CBR → CBZ conversion failed</b>"),
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
            ..
        } => {
            [
                format!("✅ <b>Metadata linked</b>"),
                String::new(),
                format!("📚 <b>Series:</b> {series_name}"),
                format!("🔗 <b>Provider:</b> {provider}"),
            ]
            .join("\n")
        }
        NotificationEvent::MetadataBatchCompleted {
            library_name,
            total_series,
            processed,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            [
                format!("✅ <b>Metadata batch completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                format!("📊 <b>Processed:</b> {processed}/{total_series} series"),
            ]
            .join("\n")
        }
        NotificationEvent::MetadataBatchFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                format!("🚨 <b>Metadata batch failed</b>"),
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
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let mut lines = vec![
                format!("✅ <b>Metadata refresh completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("📊 <b>Results</b>"),
                format!("  🔄 Updated: <b>{refreshed}</b>"),
                format!("  ▪️ Unchanged: <b>{unchanged}</b>"),
            ];
            if *errors > 0 {
                lines.push(format!("  ⚠️ Errors: <b>{errors}</b>"));
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
                format!("🚨 <b>Metadata refresh failed</b>"),
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
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            [
                format!("✅ <b>Reading status match completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("📊 <b>Results</b>"),
                format!("  🔗 Linked: <b>{linked}</b> / <b>{total_series}</b> series"),
            ]
            .join("\n")
        }
        NotificationEvent::ReadingStatusMatchFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                format!("🚨 <b>Reading status match failed</b>"),
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
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            [
                format!("✅ <b>Reading status push completed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("📊 <b>Results</b>"),
                format!("  ⬆️ Pushed: <b>{pushed}</b> / <b>{total_series}</b> series"),
            ]
            .join("\n")
        }
        NotificationEvent::ReadingStatusPushFailed {
            library_name,
            error,
        } => {
            let lib = library_name.as_deref().unwrap_or("All libraries");
            let err = truncate(error, 200);
            [
                format!("🚨 <b>Reading status push failed</b>"),
                String::new(),
                format!("📂 <b>Library:</b> {lib}"),
                String::new(),
                format!("💬 <code>{err}</code>"),
            ]
            .join("\n")
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

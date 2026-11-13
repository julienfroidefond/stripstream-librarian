//! Thumbnail configuration and its single precedence.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use tracing::warn;

use stripstream_core::settings::load_setting;

use crate::store::ThumbnailStore;

/// `app_settings` key holding the thumbnail configuration.
pub const SETTINGS_KEY: &str = "thumbnail";

/// Directory used when the setting does not name one.
pub const DEFAULT_DIRECTORY: &str = "/data/thumbnails";

/// Output format of generated thumbnails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailFormat {
    /// Re-encode in the source format (PNG stays PNG, everything else JPEG).
    Original,
    Webp,
    Jpeg,
    Png,
}

impl ThumbnailFormat {
    /// Parse the `format` setting. Unknown values fall back to WebP.
    pub fn parse(value: &str) -> Self {
        match value {
            "original" => Self::Original,
            "jpeg" | "jpg" => Self::Jpeg,
            "png" => Self::Png,
            _ => Self::Webp,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Webp => "webp",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
        }
    }
}

/// Thumbnail configuration shared by the API and the indexer.
///
/// Precedence: each field of the `thumbnail` row of `app_settings` (edited hot
/// from the backoffice) overrides the built-in [`Default`]; a missing row or a
/// missing/invalid field uses the default. Environment variables are not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailSettings {
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub format: ThumbnailFormat,
    pub directory: PathBuf,
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 300,
            height: 400,
            quality: 80,
            format: ThumbnailFormat::Webp,
            directory: PathBuf::from(DEFAULT_DIRECTORY),
        }
    }
}

impl ThumbnailSettings {
    /// Read the `thumbnail` setting. Fails only when the database read fails.
    pub async fn load(pool: &PgPool) -> Result<Self> {
        let value = load_setting::<Value>(pool, SETTINGS_KEY).await?;
        Ok(Self::from_value(value.as_ref()))
    }

    /// Like [`Self::load`], but falls back to the defaults (with a warning)
    /// when the database cannot be read — for background jobs and diagnostics
    /// that must keep going.
    pub async fn load_or_default(pool: &PgPool) -> Self {
        match Self::load(pool).await {
            Ok(settings) => settings,
            Err(error) => {
                warn!(target: "thumbnail", "[THUMBNAIL] cannot read thumbnail settings, using defaults: {error:#}");
                Self::default()
            }
        }
    }

    /// Apply the stored JSON over the defaults, field by field.
    pub fn from_value(value: Option<&Value>) -> Self {
        let defaults = Self::default();
        let Some(value) = value else {
            return defaults;
        };
        let u32_field = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v > 0)
        };
        let directory = value
            .get("directory")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        Self {
            enabled: value
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(defaults.enabled),
            width: u32_field("width").unwrap_or(defaults.width),
            height: u32_field("height").unwrap_or(defaults.height),
            quality: value
                .get("quality")
                .and_then(Value::as_u64)
                .map(|v| v.clamp(1, 100) as u8)
                .unwrap_or(defaults.quality),
            format: value
                .get("format")
                .and_then(Value::as_str)
                .map(ThumbnailFormat::parse)
                .unwrap_or(defaults.format),
            directory: directory.unwrap_or(defaults.directory),
        }
    }

    /// The store writing into this configuration's directory.
    pub fn store(&self) -> ThumbnailStore {
        ThumbnailStore::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_row_uses_defaults() {
        assert_eq!(
            ThumbnailSettings::from_value(None),
            ThumbnailSettings::default()
        );
    }

    #[test]
    fn stored_fields_override_defaults_one_by_one() {
        let settings = ThumbnailSettings::from_value(Some(&json!({
            "width": 200,
            "format": "png",
            "directory": "/srv/thumbs",
        })));
        assert_eq!(settings.width, 200);
        assert_eq!(settings.format, ThumbnailFormat::Png);
        assert_eq!(settings.directory, PathBuf::from("/srv/thumbs"));
        // Untouched fields keep their defaults.
        assert_eq!(settings.height, 400);
        assert_eq!(settings.quality, 80);
        assert!(settings.enabled);
    }

    #[test]
    fn invalid_fields_fall_back_to_defaults() {
        let settings = ThumbnailSettings::from_value(Some(&json!({
            "enabled": "yes",
            "width": 0,
            "height": -5,
            "quality": 500,
            "format": "gif",
            "directory": "  ",
        })));
        assert!(settings.enabled);
        assert_eq!(settings.width, 300);
        assert_eq!(settings.height, 400);
        assert_eq!(settings.quality, 100, "quality is clamped to 1..=100");
        assert_eq!(settings.format, ThumbnailFormat::Webp);
        assert_eq!(settings.directory, PathBuf::from(DEFAULT_DIRECTORY));
    }

    #[test]
    fn format_parsing_accepts_aliases() {
        assert_eq!(
            ThumbnailFormat::parse("original"),
            ThumbnailFormat::Original
        );
        assert_eq!(ThumbnailFormat::parse("jpg"), ThumbnailFormat::Jpeg);
        assert_eq!(ThumbnailFormat::parse("jpeg"), ThumbnailFormat::Jpeg);
        assert_eq!(ThumbnailFormat::parse("png"), ThumbnailFormat::Png);
        assert_eq!(ThumbnailFormat::parse("webp"), ThumbnailFormat::Webp);
    }

    async fn set_thumbnail_row(pool: &PgPool, value: Value) {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ($1, $2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(SETTINGS_KEY)
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn load_reads_the_seeded_row(pool: PgPool) {
        // Migration 0009 seeds the row with the built-in defaults.
        let settings = ThumbnailSettings::load(&pool).await.unwrap();
        assert_eq!(settings, ThumbnailSettings::default());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn database_row_wins_over_defaults(pool: PgPool) {
        set_thumbnail_row(
            &pool,
            json!({ "enabled": false, "quality": 55, "directory": "/tmp/stl-thumbs" }),
        )
        .await;

        let settings = ThumbnailSettings::load(&pool).await.unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.quality, 55);
        assert_eq!(settings.directory, PathBuf::from("/tmp/stl-thumbs"));
        assert_eq!(settings.width, 300);
        assert_eq!(settings.format, ThumbnailFormat::Webp);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn deleted_row_falls_back_to_defaults(pool: PgPool) {
        sqlx::query("DELETE FROM app_settings WHERE key = $1")
            .bind(SETTINGS_KEY)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            ThumbnailSettings::load_or_default(&pool).await,
            ThumbnailSettings::default()
        );
    }
}

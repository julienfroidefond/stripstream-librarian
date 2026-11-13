//! Typed schema of the `app_settings.limits` row (ADR 0026).
//!
//! Every field of the row is declared once here, with its default and its
//! accepted range. [`Limits::parse`] is the write-side validation (it rejects
//! the whole value on the first bad field set) and [`Limits::from_stored`] the
//! read side (a bad stored field falls back to its default). Both go through
//! the same per-field rule, so the stored JSON and the running process cannot
//! disagree on what a value means, and no consumer re-clamps a value it reads.
//!
//! Fields are `NonZero*`: a `0` that would build a `Semaphore::new(0)` (every
//! render or download blocked forever) or a zero-length timeout cannot be
//! represented at all.

use std::fmt;
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

use serde_json::{Map, Value};
use sqlx::PgPool;
use stripstream_core::settings::load_setting;
use tracing::warn;

/// Key of the row in `app_settings`.
pub const LIMITS_KEY: &str = "limits";

/// Default and accepted range of one `limits` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub name: &'static str,
    pub default: u64,
    pub min: u64,
    pub max: u64,
}

impl Spec {
    const fn new(name: &'static str, default: u64, min: u64, max: u64) -> Self {
        Self {
            name,
            default,
            min,
            max,
        }
    }
}

/// Highest accepted value for any per-queue download concurrency.
const MAX_DOWNLOAD_CONCURRENCY: u64 = 16;

/// Pages rendered at once by the API (8 by default).
pub const CONCURRENT_RENDERS: Spec = Spec::new("concurrent_renders", 8, 1, u32::MAX as u64);
/// Read requests admitted per client and per second.
pub const RATE_LIMIT_PER_SECOND: Spec = Spec::new("rate_limit_per_second", 120, 1, u32::MAX as u64);
/// Page render timeout.
pub const TIMEOUT_SECONDS: Spec = Spec::new("timeout_seconds", 12, 1, 24 * 60 * 60);
/// Concurrent Telegram book downloads.
pub const CONCURRENT_TELEGRAM_DOWNLOADS: Spec = Spec::new(
    "concurrent_telegram_downloads",
    2,
    1,
    MAX_DOWNLOAD_CONCURRENCY,
);
/// Concurrent direct (DDL provider) downloads.
pub const CONCURRENT_DDL_PROVIDER_DOWNLOADS: Spec = Spec::new(
    "concurrent_ddl_provider_downloads",
    2,
    1,
    MAX_DOWNLOAD_CONCURRENCY,
);
/// Concurrent AllDebrid magnet transfers in the poller.
pub const CONCURRENT_ALLDEBRID_DOWNLOADS: Spec = Spec::new(
    "concurrent_alldebrid_downloads",
    4,
    1,
    MAX_DOWNLOAD_CONCURRENCY,
);
/// Connections opened per Telegram download. One Telegram socket caps around
/// 2 MB/s, so a handful of them makes large files finish in reasonable time;
/// past 8 Telegram throttles and throughput collapses.
pub const TELEGRAM_DOWNLOAD_CONNECTIONS: Spec = Spec::new("telegram_download_connections", 4, 1, 8);
/// Connections opened per direct (HTTP) download.
pub const DDL_DOWNLOAD_CONNECTIONS: Spec = Spec::new("ddl_download_connections", 4, 1, 16);
/// Block retries before a transfer is abandoned.
pub const DOWNLOAD_MAX_ATTEMPTS: Spec = Spec::new("download_max_attempts", 5, 1, 20);
/// How long a single ranged request may stay silent before it is retried.
pub const DOWNLOAD_STALL_TIMEOUT_SECONDS: Spec =
    Spec::new("download_stall_timeout_seconds", 180, 10, 3600);
/// Whole-transfer ceiling for a direct (HTTP) download.
pub const DOWNLOAD_TIMEOUT_MINUTES: Spec = Spec::new("download_timeout_minutes", 30, 1, 24 * 60);
/// How long an in-flight AllDebrid magnet may stay at zero bytes before it is
/// abandoned as unreachable (dead torrent or blocked tracker).
pub const ALLDEBRID_STALL_TIMEOUT_MINUTES: Spec =
    Spec::new("alldebrid_stall_timeout_minutes", 30, 1, 24 * 60);

/// One rejected field of a `limits` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{LIMITS_KEY}.{} {}", self.field, self.message)
    }
}

/// The decoded `app_settings.limits` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub concurrent_renders: NonZeroUsize,
    pub rate_limit_per_second: NonZeroU32,
    pub timeout_seconds: NonZeroU64,
    pub concurrent_telegram_downloads: NonZeroUsize,
    pub concurrent_ddl_provider_downloads: NonZeroUsize,
    pub concurrent_alldebrid_downloads: NonZeroUsize,
    pub telegram_download_connections: NonZeroUsize,
    pub ddl_download_connections: NonZeroUsize,
    pub download_max_attempts: NonZeroUsize,
    pub download_stall_timeout_seconds: NonZeroU64,
    pub download_timeout_minutes: NonZeroU64,
    pub alldebrid_stall_timeout_minutes: NonZeroU64,
}

impl Default for Limits {
    fn default() -> Self {
        Self::decode(&Map::new()).0
    }
}

impl Limits {
    /// Strict decoding, used before a value is written: any present field that
    /// is not an integer within its range is reported. Absent fields take their
    /// default; unknown fields are ignored.
    pub fn parse(value: &Value) -> Result<Self, Vec<FieldError>> {
        let Some(obj) = value.as_object() else {
            return Err(vec![FieldError {
                field: "*",
                message: "must be a JSON object".to_string(),
            }]);
        };
        let (limits, errors) = Self::decode(obj);
        if errors.is_empty() {
            Ok(limits)
        } else {
            Err(errors)
        }
    }

    /// Lenient decoding of a stored row: a missing or non-object row yields the
    /// defaults, and every invalid field falls back to its own default (with a
    /// warning) instead of discarding the valid ones.
    pub fn from_stored(value: Option<&Value>) -> Self {
        let Some(obj) = value.and_then(Value::as_object) else {
            if let Some(other) = value.filter(|v| !v.is_null()) {
                warn!("app_settings.{LIMITS_KEY} is not an object ({other}); using defaults");
            }
            return Self::default();
        };
        let (limits, errors) = Self::decode(obj);
        for error in errors {
            warn!("ignoring stored {error}; using the default");
        }
        limits
    }

    /// Reads the row, falling back to the defaults when it is missing or
    /// unreadable.
    pub async fn load(pool: &PgPool) -> Self {
        let value = load_setting::<Value>(pool, LIMITS_KEY).await.ok().flatten();
        Self::from_stored(value.as_ref())
    }

    fn decode(obj: &Map<String, Value>) -> (Self, Vec<FieldError>) {
        let mut errors = Vec::new();
        let mut field = |spec: &Spec| read_field(obj, spec, &mut errors);
        let limits = Self {
            concurrent_renders: usize_of(field(&CONCURRENT_RENDERS)),
            rate_limit_per_second: u32_of(field(&RATE_LIMIT_PER_SECOND)),
            timeout_seconds: field(&TIMEOUT_SECONDS),
            concurrent_telegram_downloads: usize_of(field(&CONCURRENT_TELEGRAM_DOWNLOADS)),
            concurrent_ddl_provider_downloads: usize_of(field(&CONCURRENT_DDL_PROVIDER_DOWNLOADS)),
            concurrent_alldebrid_downloads: usize_of(field(&CONCURRENT_ALLDEBRID_DOWNLOADS)),
            telegram_download_connections: usize_of(field(&TELEGRAM_DOWNLOAD_CONNECTIONS)),
            ddl_download_connections: usize_of(field(&DDL_DOWNLOAD_CONNECTIONS)),
            download_max_attempts: usize_of(field(&DOWNLOAD_MAX_ATTEMPTS)),
            download_stall_timeout_seconds: field(&DOWNLOAD_STALL_TIMEOUT_SECONDS),
            download_timeout_minutes: field(&DOWNLOAD_TIMEOUT_MINUTES),
            alldebrid_stall_timeout_minutes: field(&ALLDEBRID_STALL_TIMEOUT_MINUTES),
        };
        (limits, errors)
    }
}

/// The single per-field rule: absent → default; present → an integer within
/// `spec.min..=spec.max`, otherwise an error and the default.
fn read_field(obj: &Map<String, Value>, spec: &Spec, errors: &mut Vec<FieldError>) -> NonZeroU64 {
    let value = match obj.get(spec.name) {
        None => spec.default,
        Some(raw) => match raw.as_u64() {
            Some(n) if (spec.min..=spec.max).contains(&n) => n,
            Some(n) => {
                errors.push(FieldError {
                    field: spec.name,
                    message: out_of_range(spec, n),
                });
                spec.default
            }
            None => {
                errors.push(FieldError {
                    field: spec.name,
                    message: "must be a positive integer".to_string(),
                });
                spec.default
            }
        },
    };
    // Every spec has `min >= 1` and `default >= min`.
    NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN)
}

fn out_of_range(spec: &Spec, n: u64) -> String {
    if n < spec.min {
        if spec.min == 1 {
            "must be greater than 0".to_string()
        } else {
            format!("must be at least {}", spec.min)
        }
    } else {
        format!("must be at most {}", spec.max)
    }
}

fn usize_of(n: NonZeroU64) -> NonZeroUsize {
    usize::try_from(n.get())
        .ok()
        .and_then(NonZeroUsize::new)
        .unwrap_or(NonZeroUsize::MAX)
}

fn u32_of(n: NonZeroU64) -> NonZeroU32 {
    u32::try_from(n.get())
        .ok()
        .and_then(NonZeroU32::new)
        .unwrap_or(NonZeroU32::MAX)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ALL: [Spec; 12] = [
        CONCURRENT_RENDERS,
        RATE_LIMIT_PER_SECOND,
        TIMEOUT_SECONDS,
        CONCURRENT_TELEGRAM_DOWNLOADS,
        CONCURRENT_DDL_PROVIDER_DOWNLOADS,
        CONCURRENT_ALLDEBRID_DOWNLOADS,
        TELEGRAM_DOWNLOAD_CONNECTIONS,
        DDL_DOWNLOAD_CONNECTIONS,
        DOWNLOAD_MAX_ATTEMPTS,
        DOWNLOAD_STALL_TIMEOUT_SECONDS,
        DOWNLOAD_TIMEOUT_MINUTES,
        ALLDEBRID_STALL_TIMEOUT_MINUTES,
    ];

    fn fields(errors: &[FieldError]) -> Vec<&'static str> {
        errors.iter().map(|e| e.field).collect()
    }

    #[test]
    fn every_spec_is_consistent() {
        for spec in ALL {
            assert!(spec.min >= 1, "{} accepts 0", spec.name);
            assert!(
                (spec.min..=spec.max).contains(&spec.default),
                "{} default out of range",
                spec.name
            );
        }
    }

    #[test]
    fn empty_object_yields_the_defaults() {
        let limits = Limits::parse(&json!({})).unwrap();
        assert_eq!(limits, Limits::default());
        assert_eq!(limits.concurrent_renders.get(), 8);
        assert_eq!(limits.rate_limit_per_second.get(), 120);
        assert_eq!(limits.timeout_seconds.get(), 12);
        assert_eq!(limits.concurrent_telegram_downloads.get(), 2);
        assert_eq!(limits.concurrent_ddl_provider_downloads.get(), 2);
        assert_eq!(limits.concurrent_alldebrid_downloads.get(), 4);
        assert_eq!(limits.telegram_download_connections.get(), 4);
        assert_eq!(limits.ddl_download_connections.get(), 4);
        assert_eq!(limits.download_max_attempts.get(), 5);
        assert_eq!(limits.download_stall_timeout_seconds.get(), 180);
        assert_eq!(limits.download_timeout_minutes.get(), 30);
        assert_eq!(limits.alldebrid_stall_timeout_minutes.get(), 30);
    }

    #[test]
    fn parse_reads_every_field_and_ignores_unknown_ones() {
        let limits = Limits::parse(&json!({
            "concurrent_renders": 3,
            "rate_limit_per_second": 7,
            "timeout_seconds": 42,
            "concurrent_telegram_downloads": 3,
            "concurrent_ddl_provider_downloads": 5,
            "concurrent_alldebrid_downloads": 6,
            "telegram_download_connections": 7,
            "ddl_download_connections": 9,
            "download_max_attempts": 11,
            "download_stall_timeout_seconds": 240,
            "download_timeout_minutes": 90,
            "alldebrid_stall_timeout_minutes": 45,
            "something_else": "kept by the row, ignored here",
        }))
        .unwrap();

        assert_eq!(limits.concurrent_renders.get(), 3);
        assert_eq!(limits.rate_limit_per_second.get(), 7);
        assert_eq!(limits.timeout_seconds.get(), 42);
        assert_eq!(limits.concurrent_telegram_downloads.get(), 3);
        assert_eq!(limits.concurrent_ddl_provider_downloads.get(), 5);
        assert_eq!(limits.concurrent_alldebrid_downloads.get(), 6);
        assert_eq!(limits.telegram_download_connections.get(), 7);
        assert_eq!(limits.ddl_download_connections.get(), 9);
        assert_eq!(limits.download_max_attempts.get(), 11);
        assert_eq!(limits.download_stall_timeout_seconds.get(), 240);
        assert_eq!(limits.download_timeout_minutes.get(), 90);
        assert_eq!(limits.alldebrid_stall_timeout_minutes.get(), 45);
    }

    #[test]
    fn parse_accepts_both_bounds_and_rejects_just_outside() {
        for spec in ALL {
            for ok in [spec.min, spec.max] {
                assert!(
                    Limits::parse(&json!({ spec.name: ok })).is_ok(),
                    "{} = {ok} must be accepted",
                    spec.name
                );
            }
            let below = Limits::parse(&json!({ spec.name: spec.min - 1 })).unwrap_err();
            assert_eq!(fields(&below), [spec.name]);
            if spec.max < u64::MAX {
                let above = Limits::parse(&json!({ spec.name: spec.max + 1 })).unwrap_err();
                assert_eq!(fields(&above), [spec.name]);
            }
        }
    }

    #[test]
    fn parse_reports_every_bad_field_with_a_readable_message() {
        let errors = Limits::parse(&json!({
            "concurrent_renders": 0,
            "timeout_seconds": "soon",
            "telegram_download_connections": 9,
            "download_stall_timeout_seconds": 5,
            "rate_limit_per_second": 10,
        }))
        .unwrap_err();

        let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
        assert_eq!(
            messages,
            [
                "limits.concurrent_renders must be greater than 0",
                "limits.timeout_seconds must be a positive integer",
                "limits.telegram_download_connections must be at most 8",
                "limits.download_stall_timeout_seconds must be at least 10",
            ]
        );
    }

    #[test]
    fn parse_rejects_negative_float_null_and_non_object_values() {
        for bad in [json!(-1), json!(1.5), Value::Null, json!("4")] {
            assert!(
                Limits::parse(&json!({ "concurrent_renders": bad })).is_err(),
                "{bad} must be rejected"
            );
        }
        assert_eq!(fields(&Limits::parse(&json!([1])).unwrap_err()), ["*"]);
    }

    #[test]
    fn from_stored_falls_back_field_by_field() {
        let limits = Limits::from_stored(Some(&json!({
            "concurrent_renders": 0,
            "telegram_download_connections": 99,
            "ddl_download_connections": "many",
            "rate_limit_per_second": 7,
        })));
        let default = Limits::default();
        assert_eq!(limits.concurrent_renders, default.concurrent_renders);
        assert_eq!(
            limits.telegram_download_connections,
            default.telegram_download_connections
        );
        assert_eq!(
            limits.ddl_download_connections,
            default.ddl_download_connections
        );
        // Valid fields are kept even when siblings are invalid.
        assert_eq!(limits.rate_limit_per_second.get(), 7);
    }

    #[test]
    fn from_stored_without_an_object_yields_the_defaults() {
        assert_eq!(Limits::from_stored(None), Limits::default());
        assert_eq!(Limits::from_stored(Some(&Value::Null)), Limits::default());
        assert_eq!(Limits::from_stored(Some(&json!("nope"))), Limits::default());
    }

    #[test]
    fn migration_seed_row_is_valid() {
        // `0008_add_settings.sql` seeds this exact value.
        let seed =
            json!({"concurrent_renders": 4, "timeout_seconds": 12, "rate_limit_per_second": 120});
        assert_eq!(Limits::parse(&seed).unwrap().concurrent_renders.get(), 4);
    }
}

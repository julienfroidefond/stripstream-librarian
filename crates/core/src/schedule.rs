/// Valid schedule modes for library monitoring.
pub const VALID_SCHEDULE_MODES: &[&str] = &["manual", "hourly", "daily", "weekly"];

/// Convert a schedule mode to its interval in minutes.
/// Returns the interval for known modes, defaults to daily (1440) for unknown values.
pub fn mode_to_interval_minutes(mode: &str) -> i64 {
    match mode {
        "hourly" => 60,
        "daily" => 1440,
        "weekly" => 10080,
        _ => 1440,
    }
}

/// Validate that a mode string is one of the accepted schedule modes.
/// Returns an error message if invalid.
pub fn validate_schedule_mode(mode: &str) -> Result<(), &'static str> {
    if VALID_SCHEDULE_MODES.contains(&mode) {
        Ok(())
    } else {
        Err("must be one of: manual, hourly, daily, weekly")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_known_modes() {
        assert_eq!(mode_to_interval_minutes("hourly"), 60);
        assert_eq!(mode_to_interval_minutes("daily"), 1440);
        assert_eq!(mode_to_interval_minutes("weekly"), 10080);
    }

    #[test]
    fn interval_unknown_defaults_to_daily() {
        assert_eq!(mode_to_interval_minutes("manual"), 1440);
        assert_eq!(mode_to_interval_minutes("unknown"), 1440);
        assert_eq!(mode_to_interval_minutes(""), 1440);
    }

    #[test]
    fn validate_accepts_valid() {
        assert!(validate_schedule_mode("manual").is_ok());
        assert!(validate_schedule_mode("hourly").is_ok());
        assert!(validate_schedule_mode("daily").is_ok());
        assert!(validate_schedule_mode("weekly").is_ok());
    }

    #[test]
    fn validate_rejects_invalid() {
        assert!(validate_schedule_mode("monthly").is_err());
        assert!(validate_schedule_mode("").is_err());
        assert!(validate_schedule_mode("HOURLY").is_err());
    }
}

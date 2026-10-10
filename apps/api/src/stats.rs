mod breakdown;
mod get_stats;
mod overview;
mod period;
mod reading;
mod types;

pub use breakdown::get_stats_breakdown;
pub use get_stats::get_stats;
pub use overview::get_stats_overview;
pub use reading::get_reading_overview;
pub use types::*;

// Re-export the `utoipa` path glue generated next to the handlers so the
// OpenAPI document can reference them through `crate::stats::…`.
pub use get_stats::__path_get_stats;
pub use reading::__path_get_reading_overview;

#[cfg(test)]
#[path = "tests/stats.rs"]
mod tests;

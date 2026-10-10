use std::time::Duration;

use crate::cache::TtlCache;

use super::types::{StatsBreakdownResponse, StatsOverviewResponse, StatsResponse};

/// How long a built dashboard response stays cached. Long enough to absorb the
/// Backoffice's batched polling of `/stats`, `/stats/overview` and
/// `/stats/breakdown`, short enough that freshly indexed books appear promptly.
const STATS_CACHE_TTL: Duration = Duration::from_secs(20);

/// TTL caches for the three dashboard endpoints. Each endpoint has its own key
/// shape: the full stats depend on the requested period, the overview and
/// breakdown only on the authenticated user.
pub struct StatsCache {
    pub(crate) full: TtlCache<(Option<uuid::Uuid>, String), StatsResponse>,
    pub(crate) overview: TtlCache<Option<uuid::Uuid>, StatsOverviewResponse>,
    pub(crate) breakdown: TtlCache<Option<uuid::Uuid>, StatsBreakdownResponse>,
}

impl StatsCache {
    pub fn new() -> Self {
        Self {
            full: TtlCache::new(STATS_CACHE_TTL),
            overview: TtlCache::new(STATS_CACHE_TTL),
            breakdown: TtlCache::new(STATS_CACHE_TTL),
        }
    }
}

impl Default for StatsCache {
    fn default() -> Self {
        Self::new()
    }
}

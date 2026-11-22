//! The single read model of the series catalogue.
//!
//! Every surface returning a [`SeriesItem`] — the cross-library list, the
//! per-library list, favourites, ongoing series and the series detail — reads
//! through [`query`]. The SQL, the projection and the filters live here so a
//! new column is added once (in [`SeriesItem`] and in the row projection)
//! instead of being silently left `None` by one of five copies.
//!
//! Handlers translate their query DTO into a [`SeriesRequest`] and the
//! resulting [`CatalogPage`] into their own response type; they never build
//! series SQL.

mod request;
mod row;
mod sql;

use sqlx::PgPool;
use uuid::Uuid;

use super::SeriesItem;
use crate::error::ApiError;

/// Reading state of a series, derived from its tome counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingStatus {
    Unread,
    Reading,
    Read,
}

impl ReadingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Reading => "reading",
            Self::Read => "read",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "unread" => Some(Self::Unread),
            "reading" => Some(Self::Reading),
            "read" => Some(Self::Read),
            _ => None,
        }
    }
}

/// A metadata gap the catalogue can filter on: the variant names the field
/// the series is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gap {
    Description,
    Genre,
    Authors,
    Publishers,
    Year,
    Cover,
    CommunityScore,
}

impl Gap {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "no_description" => Some(Self::Description),
            "no_genre" => Some(Self::Genre),
            "no_authors" => Some(Self::Authors),
            "no_publishers" => Some(Self::Publishers),
            "no_year" => Some(Self::Year),
            "no_cover" => Some(Self::Cover),
            "no_community_score" => Some(Self::CommunityScore),
            _ => None,
        }
    }
}

/// The approved-provider link a series must (or must not) have.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MetadataLink {
    #[default]
    Any,
    Linked,
    Unlinked,
    Provider(String),
}

impl MetadataLink {
    /// Any non-reserved value is a provider name, as before the typed filter.
    fn parse(value: &str) -> Self {
        match value {
            "linked" => Self::Linked,
            "unlinked" => Self::Unlinked,
            other => Self::Provider(other.to_string()),
        }
    }
}

/// A book volume type the series must own at least one of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeType {
    Regular,
    Hs,
    Oneshot,
    Integral,
}

impl VolumeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Hs => "hs",
            Self::Oneshot => "oneshot",
            Self::Integral => "integral",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "regular" => Some(Self::Regular),
            "hs" => Some(Self::Hs),
            "oneshot" => Some(Self::Oneshot),
            "integral" => Some(Self::Integral),
            _ => None,
        }
    }
}

/// The current user's rating filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RatingFilter {
    #[default]
    Any,
    RatedOnly,
    Unrated,
}

impl RatingFilter {
    fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value {
            None | Some("false") | Some("0") => Ok(Self::Any),
            Some("true") | Some("1") => Ok(Self::RatedOnly),
            Some("unrated") => Ok(Self::Unrated),
            Some(other) => Err(ApiError::bad_request(format!(
                "invalid rated_only '{other}': expected true or unrated"
            ))),
        }
    }
}

/// Order of the result page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeriesSort {
    #[default]
    Title,
    Latest,
    ReleaseDate,
    CommunityScore,
    /// Most recent reading activity first. Owned by [`SeriesScope::Ongoing`],
    /// which always uses it; it is not accepted from a query string because a
    /// series without progress has no activity to sort on.
    Activity,
}

impl SeriesSort {
    fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value {
            None | Some("title") => Ok(Self::Title),
            Some("latest") => Ok(Self::Latest),
            Some("release_date") => Ok(Self::ReleaseDate),
            Some("community_score") => Ok(Self::CommunityScore),
            Some(other) => Err(ApiError::bad_request(format!(
                "invalid sort '{other}': expected title, latest, release_date or community_score"
            ))),
        }
    }
}

/// Where the rows come from.
///
/// A scope owns both the series set and its order; filters only narrow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesScope {
    /// Every series, optionally restricted to one library.
    All { library_id: Option<Uuid> },
    /// One series, by id: the detail endpoint. Zero rows means "not found",
    /// which also covers a series hidden by a genre restriction.
    One { series_id: Uuid },
    /// The series the reader favourited, most recently favourited first.
    Favorites,
    /// The series the reader started but did not finish, most recent reading
    /// activity first. Series the reader set aside are excluded.
    Ongoing,
}

/// Filters applied to the scoped series set.
#[derive(Debug, Clone, Default)]
pub struct SeriesFilters {
    pub q: Option<String>,
    pub reading_status: Vec<ReadingStatus>,
    pub series_status: Option<String>,
    pub has_missing: bool,
    pub metadata_link: MetadataLink,
    pub gap: Option<Gap>,
    pub author: Option<String>,
    pub has_books: bool,
    pub no_books: bool,
    pub shelved: Option<bool>,
    pub genre: Option<String>,
    pub volume_type: Option<VolumeType>,
    pub rating: RatingFilter,
}

/// A fully typed catalogue read.
#[derive(Debug, Clone)]
pub struct SeriesRequest {
    pub scope: SeriesScope,
    pub filters: SeriesFilters,
    pub sort: SeriesSort,
    /// 1-indexed.
    pub page: i64,
    pub limit: i64,
    /// Whether the scope is paged. Whole-set readers (favourites) return every
    /// match in the scope's own order and ignore `page`/`limit`.
    pub paginated: bool,
    /// The reader whose progress, shelf and rating are projected.
    pub user_id: Option<Uuid>,
}

/// One page of the catalogue.
pub struct CatalogPage {
    pub items: Vec<SeriesItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

/// Run a catalogue read.
pub async fn query(pool: &PgPool, request: SeriesRequest) -> Result<CatalogPage, ApiError> {
    let (sql, params) = sql::build(&request);

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
    for param in params {
        query = param.bind(query);
    }
    let rows = query.fetch_all(pool).await?;

    let total: i64 = rows
        .first()
        .map(|row| sqlx::Row::get(row, "total_count"))
        .unwrap_or(0);

    Ok(CatalogPage {
        items: rows.iter().filter_map(row::series_item).collect(),
        total,
        page: request.page,
        limit: request.limit,
    })
}

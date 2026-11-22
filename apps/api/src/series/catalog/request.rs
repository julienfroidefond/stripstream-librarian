use uuid::Uuid;

use super::{
    MetadataLink, RatingFilter, ReadingStatus, SeriesFilters, SeriesRequest, SeriesScope,
    SeriesSort, VolumeType,
};
use crate::error::ApiError;
use crate::series::ListAllSeriesQuery;

impl SeriesRequest {
    /// Translate the `/series` query DTO into a typed catalogue read.
    ///
    /// Invalid values are rejected instead of silently ignored: the previous
    /// implementation fell back to `title` for an unknown `sort` and dropped an
    /// unknown `gap`/`volume_type`, which made a client believe a filter was
    /// applied.
    pub fn from_list_all(
        query: ListAllSeriesQuery,
        user_id: Option<Uuid>,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            scope: SeriesScope::All {
                library_id: query.library_id,
            },
            filters: SeriesFilters {
                q: query.q,
                reading_status: parse_reading_statuses(query.reading_status.as_deref())?,
                series_status: query.series_status,
                has_missing: parse_flag(query.has_missing.as_deref(), "has_missing")?,
                metadata_link: query
                    .metadata_provider
                    .as_deref()
                    .map(MetadataLink::parse)
                    .unwrap_or_default(),
                gap: query.gap.as_deref().map(parse_gap).transpose()?,
                author: query.author,
                has_books: parse_flag(query.has_books.as_deref(), "has_books")?,
                no_books: parse_flag(query.no_books.as_deref(), "no_books")?,
                shelved: parse_optional_flag(query.shelved.as_deref(), "shelved")?,
                genre: query.genre,
                volume_type: query
                    .volume_type
                    .as_deref()
                    .map(parse_volume_type)
                    .transpose()?,
                rating: RatingFilter::parse(query.rated_only.as_deref())?,
            },
            sort: SeriesSort::parse(query.sort.as_deref())?,
            page: query.page.unwrap_or(1).max(1),
            limit: query.limit.unwrap_or(50).clamp(1, 200),
            paginated: true,
            user_id,
        })
    }

    /// The series detail: one row, or none when the id is unknown or hidden by
    /// a genre restriction.
    pub fn for_one(series_id: Uuid, user_id: Option<Uuid>) -> Self {
        Self {
            scope: SeriesScope::One { series_id },
            filters: SeriesFilters::default(),
            sort: SeriesSort::Title,
            page: 1,
            limit: 1,
            paginated: true,
            user_id,
        }
    }

    /// The reader's favourites, all of them: the scope orders them by the time
    /// they were favourited.
    pub fn for_favorites(user_id: Uuid) -> Self {
        Self {
            scope: SeriesScope::Favorites,
            filters: SeriesFilters::default(),
            sort: SeriesSort::Title,
            page: 1,
            limit: 0,
            paginated: false,
            user_id: Some(user_id),
        }
    }

    /// The reader's ongoing series, most recent activity first. The sort is
    /// owned by the scope: a caller cannot ask for another one.
    pub fn for_ongoing(user_id: Uuid, limit: i64) -> Self {
        Self {
            scope: SeriesScope::Ongoing,
            filters: SeriesFilters::default(),
            sort: SeriesSort::Activity,
            page: 1,
            limit,
            paginated: true,
            user_id: Some(user_id),
        }
    }
}

/// A blank `reading_status` means "no filter", not "match nothing".
fn parse_reading_statuses(raw: Option<&str>) -> Result<Vec<ReadingStatus>, ApiError> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };

    raw.split(',')
        .map(|value| {
            let value = value.trim();
            ReadingStatus::parse(value).ok_or_else(|| {
                ApiError::bad_request(format!(
                    "invalid reading_status '{value}': expected unread, reading or read"
                ))
            })
        })
        .collect()
}

fn parse_gap(value: &str) -> Result<super::Gap, ApiError> {
    super::Gap::parse(value).ok_or_else(|| {
        ApiError::bad_request(format!(
            "invalid gap '{value}': expected no_description, no_genre, no_authors, \
             no_publishers, no_year, no_cover or no_community_score"
        ))
    })
}

fn parse_volume_type(value: &str) -> Result<VolumeType, ApiError> {
    VolumeType::parse(value).ok_or_else(|| {
        ApiError::bad_request(format!(
            "invalid volume_type '{value}': expected regular, hs, oneshot or integral"
        ))
    })
}

fn parse_flag(raw: Option<&str>, name: &str) -> Result<bool, ApiError> {
    match raw {
        None | Some("false") | Some("0") => Ok(false),
        Some("true") | Some("1") => Ok(true),
        Some(other) => Err(ApiError::bad_request(format!(
            "invalid {name} '{other}': expected true or false"
        ))),
    }
}

/// `shelved=false` is a real filter (unshelved only), unlike an absent value.
fn parse_optional_flag(raw: Option<&str>, name: &str) -> Result<Option<bool>, ApiError> {
    match raw {
        None => Ok(None),
        Some("true") | Some("1") => Ok(Some(true)),
        Some("false") | Some("0") => Ok(Some(false)),
        Some(other) => Err(ApiError::bad_request(format!(
            "invalid {name} '{other}': expected true or false"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto() -> ListAllSeriesQuery {
        ListAllSeriesQuery {
            q: None,
            library_id: None,
            reading_status: None,
            series_status: None,
            has_missing: None,
            metadata_provider: None,
            gap: None,
            author: None,
            page: None,
            limit: None,
            has_books: None,
            no_books: None,
            shelved: None,
            sort: None,
            genre: None,
            volume_type: None,
            rated_only: None,
        }
    }

    #[test]
    fn parses_the_community_score_sort() {
        let request = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                sort: Some("community_score".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap();

        assert_eq!(request.sort, SeriesSort::CommunityScore);
    }

    #[test]
    fn rejects_an_unknown_sort() {
        let error = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                sort: Some("popularity".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
        assert!(error.message.contains("invalid sort 'popularity'"));
    }

    #[test]
    fn parses_a_reading_status_list() {
        let request = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                reading_status: Some("unread, reading".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap();

        assert_eq!(
            request.filters.reading_status,
            vec![ReadingStatus::Unread, ReadingStatus::Reading]
        );
    }

    #[test]
    fn rejects_an_unknown_reading_status() {
        let error = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                reading_status: Some("started".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap_err();

        assert!(error.message.contains("invalid reading_status 'started'"));
    }

    #[test]
    fn a_blank_reading_status_means_no_filter() {
        let request = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                reading_status: Some("   ".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap();

        assert!(request.filters.reading_status.is_empty());
    }

    #[test]
    fn rejects_an_unknown_volume_type() {
        let error = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                volume_type: Some("omnibus".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap_err();

        assert!(error.message.contains("invalid volume_type 'omnibus'"));
    }

    #[test]
    fn rejects_an_unknown_gap() {
        let error = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                gap: Some("no_isbn".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap_err();

        assert!(error.message.contains("invalid gap 'no_isbn'"));
    }

    #[test]
    fn shelved_false_is_a_filter_while_absent_is_not() {
        let unshelved = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                shelved: Some("false".to_string()),
                ..dto()
            },
            None,
        )
        .unwrap();
        let any = SeriesRequest::from_list_all(dto(), None).unwrap();

        assert_eq!(unshelved.filters.shelved, Some(false));
        assert_eq!(any.filters.shelved, None);
    }

    #[test]
    fn clamps_page_and_limit() {
        let request = SeriesRequest::from_list_all(
            ListAllSeriesQuery {
                page: Some(0),
                limit: Some(500),
                ..dto()
            },
            None,
        )
        .unwrap();

        assert_eq!(request.page, 1);
        assert_eq!(request.limit, 200);
    }
}

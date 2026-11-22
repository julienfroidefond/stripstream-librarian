use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::series::SeriesItem;

/// Project one catalogue row. An empty page still returns a row (the
/// `LEFT JOIN page p ON TRUE` trick) whose `series_id` is NULL: it carries the
/// total only, so it is skipped here.
pub(super) fn series_item(row: &PgRow) -> Option<SeriesItem> {
    let series_id: Option<Uuid> = row.get("series_id");
    let series_id = series_id?;

    Some(SeriesItem {
        name: row.get("name"),
        series_id,
        book_count: row.get("book_count"),
        books_read_count: row.get("books_read_count"),
        last_read_at: row.get("last_read_at"),
        first_book_id: row.get("first_book_id"),
        first_book_updated_at: row.get("first_book_updated_at"),
        library_id: row.get("library_id"),
        series_status: row.get("series_status"),
        missing_count: row.get("missing_count"),
        metadata_provider: row.get("metadata_provider"),
        anilist_id: row.get("anilist_id"),
        anilist_url: row.get("anilist_url"),
        cover_url: row.get("cover_url"),
        start_year: row.get("start_year"),
        genres: row.get::<Vec<String>, _>("genres"),
        authors: row.get::<Vec<String>, _>("authors"),
        description: row.get("description"),
        user_rating: row.get("user_rating"),
        community_score: row.get("community_score"),
        shelved_at: row.get("shelved_at"),
    })
}

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A single metadata field change: old value -> new value.
#[derive(Serialize, ToSchema)]
pub struct FieldChange {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema, Default)]
pub struct SeriesSyncReport {
    pub fields_updated: Vec<FieldChange>,
    pub fields_skipped: Vec<FieldChange>,
}

#[derive(Serialize, ToSchema)]
pub struct BookSyncReport {
    #[schema(value_type = String)]
    pub book_id: Uuid,
    pub title: String,
    pub volume: Option<i32>,
    pub fields_updated: Vec<FieldChange>,
    pub fields_skipped: Vec<FieldChange>,
}

#[derive(Serialize, ToSchema, Default)]
pub struct SyncReport {
    pub series: Option<SeriesSyncReport>,
    pub books: Vec<BookSyncReport>,
    pub books_matched: i64,
    pub books_unmatched: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub books_message: Option<String>,
}

/// A sync-managed field exposed by the metadata catalog.
#[derive(Serialize, ToSchema)]
pub struct MetadataFieldDto {
    /// JSON key used in `locked_fields` and sync reports.
    pub key: String,
    /// Whether the field can be locked against external sync.
    pub lockable: bool,
    /// Whether the field is cleared when the last approved link is deleted.
    pub clearable: bool,
}

/// Catalog of sync-managed fields, grouped by target, used by admin UIs to
/// render lock controls.
#[derive(Serialize, ToSchema)]
pub struct MetadataFieldCatalogDto {
    pub series: Vec<MetadataFieldDto>,
    pub books: Vec<MetadataFieldDto>,
}

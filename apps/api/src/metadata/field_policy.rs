use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::shared_sync::{self, LinkMetadata};
use super::types::{
    BookSyncReport, FieldChange, MetadataFieldCatalogDto, MetadataFieldDto, SeriesSyncReport,
};
use crate::metadata_providers::BookCandidate;

// ---------------------------------------------------------------------------
// Field catalog
// ---------------------------------------------------------------------------

/// How a field is merged from a provider candidate into the local row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    /// Nullable string; a non-empty candidate replaces the local value.
    Text,
    /// Nullable integer; a `Some` candidate replaces the local value.
    Int,
    /// Text array; a non-empty candidate replaces the local value.
    TextArray,
    /// Text array seeded only when the local value is empty (series genres).
    SeedTextArray,
    /// Text derived from the first element of another field (books.author).
    DerivedFirstText,
}

/// How a field is cleared when the last approved link disappears.
#[derive(Debug, Clone, Copy)]
enum ClearKind {
    Null,
    EmptyArray,
}

#[derive(Debug, Clone, Copy)]
struct FieldDescriptor {
    /// JSON key used by locks and reports.
    key: &'static str,
    /// Database column written by the policy.
    column: &'static str,
    kind: FieldKind,
    /// Lock key; `None` means the field cannot be locked. Most fields use their
    /// own key, but `books.author` is locked through `authors`.
    lock: Option<&'static str>,
    /// `None` means the field is never cleared automatically.
    clear: Option<ClearKind>,
    /// Whether the field appears in sync reports.
    report: bool,
}

const SERIES_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor {
        key: "description",
        column: "description",
        kind: FieldKind::Text,
        lock: Some("description"),
        clear: Some(ClearKind::Null),
        report: true,
    },
    FieldDescriptor {
        key: "authors",
        column: "authors",
        kind: FieldKind::TextArray,
        lock: Some("authors"),
        clear: Some(ClearKind::EmptyArray),
        report: true,
    },
    FieldDescriptor {
        key: "publishers",
        column: "publishers",
        kind: FieldKind::TextArray,
        lock: Some("publishers"),
        clear: Some(ClearKind::EmptyArray),
        report: true,
    },
    FieldDescriptor {
        key: "start_year",
        column: "start_year",
        kind: FieldKind::Int,
        lock: Some("start_year"),
        clear: Some(ClearKind::Null),
        report: true,
    },
    FieldDescriptor {
        key: "total_volumes",
        column: "total_volumes",
        kind: FieldKind::Int,
        lock: Some("total_volumes"),
        clear: Some(ClearKind::Null),
        report: true,
    },
    FieldDescriptor {
        key: "status",
        column: "status",
        kind: FieldKind::Text,
        lock: Some("status"),
        clear: Some(ClearKind::Null),
        report: true,
    },
    FieldDescriptor {
        key: "genres",
        column: "genres",
        kind: FieldKind::SeedTextArray,
        lock: None,
        clear: None,
        report: true,
    },
    FieldDescriptor {
        key: "cover_url",
        column: "cover_url",
        kind: FieldKind::Text,
        lock: Some("cover_url"),
        clear: Some(ClearKind::Null),
        report: true,
    },
];

const BOOK_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor {
        key: "summary",
        column: "summary",
        kind: FieldKind::Text,
        lock: Some("summary"),
        clear: None,
        report: true,
    },
    FieldDescriptor {
        key: "isbn",
        column: "isbn",
        kind: FieldKind::Text,
        lock: Some("isbn"),
        clear: None,
        report: true,
    },
    FieldDescriptor {
        key: "publish_date",
        column: "publish_date",
        kind: FieldKind::Text,
        lock: Some("publish_date"),
        clear: None,
        report: true,
    },
    FieldDescriptor {
        key: "language",
        column: "language",
        kind: FieldKind::Text,
        lock: Some("language"),
        clear: None,
        report: true,
    },
    FieldDescriptor {
        key: "authors",
        column: "authors",
        kind: FieldKind::TextArray,
        lock: Some("authors"),
        clear: None,
        report: true,
    },
    // Derived from authors[0]; locked and reported through `authors`.
    FieldDescriptor {
        key: "author",
        column: "author",
        kind: FieldKind::DerivedFirstText,
        lock: Some("authors"),
        clear: None,
        report: false,
    },
];

/// Catalog of sync-managed fields exposed to admin UIs. Derived-only fields
/// (`books.author`) are excluded: they follow another field's lock.
pub(crate) fn catalog() -> MetadataFieldCatalogDto {
    fn entries(descriptors: &[FieldDescriptor]) -> Vec<MetadataFieldDto> {
        descriptors
            .iter()
            .filter(|descriptor| descriptor.report)
            .map(|descriptor| MetadataFieldDto {
                key: descriptor.key.to_string(),
                lockable: descriptor.lock.is_some(),
                clearable: descriptor.clear.is_some(),
            })
            .collect()
    }

    MetadataFieldCatalogDto {
        series: entries(SERIES_FIELDS),
        books: entries(BOOK_FIELDS),
    }
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum FieldValue {
    Text(Option<String>),
    Int(Option<i32>),
    Texts(Vec<String>),
}

impl FieldValue {
    fn is_empty(&self) -> bool {
        match self {
            FieldValue::Text(value) => match value {
                Some(value) => value.is_empty(),
                None => true,
            },
            FieldValue::Int(value) => value.is_none(),
            FieldValue::Texts(value) => value.is_empty(),
        }
    }

    fn to_json(&self) -> Option<Value> {
        match self {
            FieldValue::Text(Some(value)) => Some(Value::String(value.clone())),
            FieldValue::Text(None) => None,
            FieldValue::Int(Some(value)) => Some(Value::Number((*value).into())),
            FieldValue::Int(None) => None,
            FieldValue::Texts(values) => Some(Value::Array(
                values.iter().cloned().map(Value::String).collect(),
            )),
        }
    }
}

fn empty_value(kind: FieldKind) -> FieldValue {
    match kind {
        FieldKind::Text | FieldKind::DerivedFirstText => FieldValue::Text(None),
        FieldKind::Int => FieldValue::Int(None),
        FieldKind::TextArray | FieldKind::SeedTextArray => FieldValue::Texts(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Series merge
// ---------------------------------------------------------------------------

struct SeriesFields {
    description: Option<String>,
    authors: Vec<String>,
    publishers: Vec<String>,
    start_year: Option<i32>,
    total_volumes: Option<i32>,
    status: Option<String>,
    genres: Vec<String>,
    cover_url: Option<String>,
}

impl SeriesFields {
    /// Value bound to `key`. Keeping the mapping key-driven means a reorder in
    /// `SERIES_FIELDS` cannot silently bind a value to the wrong column.
    fn value_for(&self, key: &str) -> FieldValue {
        match key {
            "description" => FieldValue::Text(self.description.clone()),
            "authors" => FieldValue::Texts(self.authors.clone()),
            "publishers" => FieldValue::Texts(self.publishers.clone()),
            "start_year" => FieldValue::Int(self.start_year),
            "total_volumes" => FieldValue::Int(self.total_volumes),
            "status" => FieldValue::Text(self.status.clone()),
            "genres" => FieldValue::Texts(self.genres.clone()),
            "cover_url" => FieldValue::Text(self.cover_url.clone()),
            other => unreachable!("unknown series field descriptor: {other}"),
        }
    }

    /// Values in the order of `SERIES_FIELDS`.
    fn values(&self) -> Vec<FieldValue> {
        SERIES_FIELDS
            .iter()
            .map(|descriptor| self.value_for(descriptor.key))
            .collect()
    }
}

fn json_non_empty_str(json: &Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(String::from)
}

fn extract_string_array(json: &Value, key: &str) -> Option<Vec<String>> {
    json.get(key)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(String::from))
                .collect()
        })
        .filter(|values: &Vec<String>| !values.is_empty())
}

/// Merge series fields across approved links: primary first, then secondaries,
/// taking the first non-empty value for each field. Genres are taken from the
/// primary only (the app / AI tagging remains the source of truth).
fn merge_series_fields(links: &[LinkMetadata]) -> SeriesFields {
    let mut ordered: Vec<&LinkMetadata> = links.iter().collect();
    ordered.sort_by_key(|link| !link.is_primary); // primary (true) first; stable

    let description = ordered
        .iter()
        .find_map(|link| json_non_empty_str(&link.metadata_json, "description"));
    let authors = ordered
        .iter()
        .find_map(|link| extract_string_array(&link.metadata_json, "authors"))
        .unwrap_or_default();
    let publishers = ordered
        .iter()
        .find_map(|link| extract_string_array(&link.metadata_json, "publishers"))
        .unwrap_or_default();
    let start_year = ordered.iter().find_map(|link| {
        link.metadata_json
            .get("start_year")
            .and_then(Value::as_i64)
            .map(|value| value as i32)
    });
    let total_volumes = ordered.iter().find_map(|link| link.total_volumes_external);
    let status = ordered
        .iter()
        .find_map(|link| json_non_empty_str(&link.metadata_json, "status"));
    let genres = ordered
        .first()
        .and_then(|link| extract_string_array(&link.metadata_json, "genres"))
        .unwrap_or_default();
    let cover_url = ordered
        .iter()
        .find_map(|link| json_non_empty_str(&link.metadata_json, "cover_url"));

    SeriesFields {
        description,
        authors,
        publishers,
        start_year,
        total_volumes,
        status,
        genres,
        cover_url,
    }
}

// ---------------------------------------------------------------------------
// SQL generation
// ---------------------------------------------------------------------------

fn series_lock_check(key: &str) -> String {
    format!("(series.locked_fields->>'{key}')::boolean IS TRUE")
}

fn book_lock_check(key: &str) -> String {
    format!("(locked_fields->>'{key}')::boolean IS TRUE")
}

fn series_update_expr(descriptor: &FieldDescriptor) -> String {
    let column = descriptor.column;
    let base = match descriptor.kind {
        FieldKind::Text => {
            format!("COALESCE(NULLIF(EXCLUDED.{column}, ''), series.{column})")
        }
        FieldKind::Int => format!("COALESCE(EXCLUDED.{column}, series.{column})"),
        FieldKind::TextArray => format!(
            "CASE WHEN array_length(EXCLUDED.{column}, 1) > 0 \
             THEN EXCLUDED.{column} ELSE series.{column} END"
        ),
        FieldKind::SeedTextArray => format!(
            "CASE WHEN array_length(series.{column}, 1) > 0 THEN series.{column} \
             WHEN array_length(EXCLUDED.{column}, 1) > 0 THEN EXCLUDED.{column} \
             ELSE series.{column} END"
        ),
        FieldKind::DerivedFirstText => {
            debug_assert!(false, "series fields do not derive text");
            format!("series.{column}")
        }
    };

    match descriptor.lock {
        Some(key) => format!(
            "CASE WHEN {} THEN series.{column} ELSE {base} END",
            series_lock_check(key)
        ),
        None => base,
    }
}

fn book_update_expr(descriptor: &FieldDescriptor, param: usize) -> String {
    let column = descriptor.column;
    let base = match descriptor.kind {
        FieldKind::Text => format!("COALESCE(NULLIF(${param}, ''), {column})"),
        FieldKind::TextArray => {
            format!("CASE WHEN CARDINALITY(${param}::text[]) > 0 THEN ${param} ELSE {column} END")
        }
        FieldKind::DerivedFirstText => format!(
            "CASE WHEN CARDINALITY(${param}::text[]) > 0 THEN ${param}[1] ELSE {column} END"
        ),
        FieldKind::Int | FieldKind::SeedTextArray => {
            debug_assert!(false, "book fields do not use this kind");
            column.to_string()
        }
    };

    match descriptor.lock {
        Some(key) => format!(
            "CASE WHEN {} THEN {column} ELSE {base} END",
            book_lock_check(key)
        ),
        None => base,
    }
}

fn series_upsert_sql() -> String {
    let columns = SERIES_FIELDS
        .iter()
        .map(|descriptor| descriptor.column)
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (3..3 + SERIES_FIELDS.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let updates = SERIES_FIELDS
        .iter()
        .map(|descriptor| format!("{} = {}", descriptor.column, series_update_expr(descriptor)))
        .collect::<Vec<_>>()
        .join(",\n            ");

    format!(
        "INSERT INTO series (id, library_id, name, {columns}, created_at, updated_at)\n\
         VALUES (gen_random_uuid(), $1, $2, {placeholders}, NOW(), NOW())\n\
         ON CONFLICT (library_id, name)\n\
         DO UPDATE SET\n            {updates},\n            updated_at = NOW()"
    )
}

fn series_select_sql() -> String {
    let columns = SERIES_FIELDS
        .iter()
        .map(|descriptor| descriptor.column)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT {columns}, locked_fields FROM series \
         WHERE library_id = $1 AND norm_text(name) = norm_text($2)"
    )
}

fn series_clear_sql() -> String {
    let sets = SERIES_FIELDS
        .iter()
        .filter_map(|descriptor| {
            let clear = descriptor.clear?;
            let clear_expr = match clear {
                ClearKind::Null => "NULL",
                ClearKind::EmptyArray => "'{}'",
            };
            Some(match descriptor.lock {
                Some(key) => format!(
                    "{} = CASE WHEN {} THEN {} ELSE {clear_expr} END",
                    descriptor.column,
                    series_lock_check(key),
                    descriptor.column
                ),
                None => format!("{} = {clear_expr}", descriptor.column),
            })
        })
        .collect::<Vec<_>>()
        .join(",\n            ");

    format!(
        "UPDATE series SET\n            {sets},\n            updated_at = NOW()\n         WHERE id = $1"
    )
}

fn book_update_sql() -> String {
    let sets = BOOK_FIELDS
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            format!(
                "{} = {}",
                descriptor.column,
                book_update_expr(descriptor, index + 2)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n            ");

    format!(
        "UPDATE books SET\n            {sets},\n            updated_at = NOW()\n         WHERE id = $1"
    )
}

fn book_select_sql() -> String {
    let columns = BOOK_FIELDS
        .iter()
        .map(|descriptor| descriptor.column)
        .collect::<Vec<_>>()
        .join(", ");
    format!("SELECT title, {columns}, locked_fields FROM books WHERE id = $1")
}

// ---------------------------------------------------------------------------
// Lock policy
// ---------------------------------------------------------------------------

/// Locks are strict booleans. Legacy JSON values are normalized by migration
/// 0139 so the generated SQL cast and this function agree.
pub(crate) fn is_field_locked(locked: &Value, field: &str) -> bool {
    locked.get(field).and_then(Value::as_bool).unwrap_or(false)
}

/// Normalize a client-supplied series `locked_fields` payload: keep only known
/// lockable keys explicitly set to `true`. Unknown keys (locks left by older UI
/// versions), non-boolean values and `false` entries are dropped.
pub(crate) fn sanitize_series_locks(locked: Option<&Value>) -> Value {
    sanitize_locks(locked, SERIES_FIELDS)
}

/// Same policy as [`sanitize_series_locks`] for book fields.
pub(crate) fn sanitize_book_locks(locked: Option<&Value>) -> Value {
    sanitize_locks(locked, BOOK_FIELDS)
}

fn sanitize_locks(locked: Option<&Value>, descriptors: &[FieldDescriptor]) -> Value {
    let mut sanitized = serde_json::Map::new();
    if let Some(Value::Object(entries)) = locked {
        for descriptor in descriptors {
            let Some(lock_key) = descriptor.lock else {
                continue;
            };
            if entries.get(lock_key).and_then(Value::as_bool) == Some(true) {
                sanitized.insert(lock_key.to_string(), Value::Bool(true));
            }
        }
    }
    Value::Object(sanitized)
}

// ---------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------

/// Normalize provider-specific status strings using the status_mappings table.
/// Returns the lowercased provider status when no mapping exists.
pub(crate) async fn normalize_series_status(pool: &PgPool, raw: &str) -> String {
    let lower = raw.to_lowercase();

    if let Ok(Some(row)) = sqlx::query_scalar::<_, String>(
        "SELECT mapped_status FROM status_mappings WHERE provider_status = $1 AND mapped_status IS NOT NULL",
    )
    .bind(&lower)
    .fetch_optional(pool)
    .await
    {
        return row;
    }

    if let Ok(Some(row)) = sqlx::query_scalar::<_, String>(
        "SELECT mapped_status FROM status_mappings WHERE $1 LIKE '%' || provider_status || '%' AND mapped_status IS NOT NULL LIMIT 1",
    )
    .bind(&lower)
    .fetch_optional(pool)
    .await
    {
        return row;
    }

    lower
}

fn read_field(row: &sqlx::postgres::PgRow, descriptor: &FieldDescriptor) -> FieldValue {
    match descriptor.kind {
        FieldKind::Text | FieldKind::DerivedFirstText => {
            FieldValue::Text(row.get::<Option<String>, _>(descriptor.column))
        }
        FieldKind::Int => FieldValue::Int(row.get::<Option<i32>, _>(descriptor.column)),
        FieldKind::TextArray | FieldKind::SeedTextArray => {
            FieldValue::Texts(row.get::<Vec<String>, _>(descriptor.column))
        }
    }
}

fn build_report(
    existing: Option<&sqlx::postgres::PgRow>,
    candidate: &[FieldValue],
    descriptors: &[FieldDescriptor],
    locked: &Value,
) -> (Vec<FieldChange>, Vec<FieldChange>) {
    let mut updated = Vec::new();
    let mut skipped = Vec::new();

    for (descriptor, new_value) in descriptors.iter().zip(candidate) {
        if !descriptor.report || new_value.is_empty() {
            continue;
        }

        let old_value = existing.map(|row| read_field(row, descriptor));
        // A series inserted by this sync has no previous value: keep the
        // report shape consistent for arrays too (no `[]` old value).
        let old_json = old_value.as_ref().and_then(FieldValue::to_json);

        // Genres are a seed, not a synced value: an existing local taxonomy
        // always wins and is never reported as skipped.
        if descriptor.kind == FieldKind::SeedTextArray
            && old_value.as_ref().is_some_and(|value| !value.is_empty())
        {
            continue;
        }

        let old_value = old_value.unwrap_or_else(|| empty_value(descriptor.kind));
        let change = FieldChange {
            field: descriptor.key.to_string(),
            old_value: old_json,
            new_value: new_value.to_json(),
        };

        if descriptor
            .lock
            .is_some_and(|key| is_field_locked(locked, key))
        {
            skipped.push(change);
        } else if old_value != *new_value {
            updated.push(change);
        }
    }

    (updated, skipped)
}

fn bind_values<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    values: &'q [FieldValue],
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    for value in values {
        query = match value {
            FieldValue::Text(value) => query.bind(value.as_deref()),
            FieldValue::Int(value) => query.bind(*value),
            FieldValue::Texts(value) => query.bind(value),
        };
    }
    query
}

/// Apply the merged series metadata, respecting locks. Returns the per-field
/// report built from the pre-update row.
pub(crate) async fn apply_series(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    links: &[LinkMetadata],
) -> Result<SeriesSyncReport, sqlx::Error> {
    let mut fields = merge_series_fields(links);
    if let Some(raw) = fields.status.clone() {
        fields.status = Some(normalize_series_status(pool, &raw).await);
    }

    let existing = sqlx::query(sqlx::AssertSqlSafe(series_select_sql()))
        .bind(library_id)
        .bind(series_name)
        .fetch_optional(pool)
        .await?;

    let locked = existing
        .as_ref()
        .map(|row| row.get::<Value, _>("locked_fields"))
        .unwrap_or_else(|| serde_json::json!({}));

    let candidate = fields.values();
    let query = sqlx::query(sqlx::AssertSqlSafe(series_upsert_sql()))
        .bind(library_id)
        .bind(series_name);
    bind_values(query, &candidate).execute(pool).await?;

    let (fields_updated, fields_skipped) =
        build_report(existing.as_ref(), &candidate, SERIES_FIELDS, &locked);

    Ok(SeriesSyncReport {
        fields_updated,
        fields_skipped,
    })
}

fn book_values(candidate: &BookCandidate) -> Vec<FieldValue> {
    BOOK_FIELDS
        .iter()
        .map(|descriptor| match descriptor.key {
            "summary" => FieldValue::Text(candidate.summary.clone()),
            "isbn" => FieldValue::Text(candidate.isbn.clone()),
            "publish_date" => FieldValue::Text(candidate.publish_date.clone()),
            "language" => FieldValue::Text(candidate.language.clone()),
            // `author` is derived from the same source as `authors`.
            "authors" | "author" => FieldValue::Texts(candidate.authors.clone()),
            other => unreachable!("unknown book field descriptor: {other}"),
        })
        .collect()
}

/// Apply merged book metadata, respecting locks. Returns `None` when the
/// candidate changes nothing (and is neither locked nor skipped).
pub(crate) async fn apply_book(
    pool: &PgPool,
    book_id: Uuid,
    candidate: &BookCandidate,
) -> Result<Option<BookSyncReport>, sqlx::Error> {
    let current = sqlx::query(sqlx::AssertSqlSafe(book_select_sql()))
        .bind(book_id)
        .fetch_one(pool)
        .await?;
    let locked = current.get::<Value, _>("locked_fields");

    let values = book_values(candidate);
    let query = sqlx::query(sqlx::AssertSqlSafe(book_update_sql())).bind(book_id);
    bind_values(query, &values).execute(pool).await?;

    let (fields_updated, fields_skipped) =
        build_report(Some(&current), &values, BOOK_FIELDS, &locked);

    if fields_updated.is_empty() && fields_skipped.is_empty() {
        return Ok(None);
    }

    Ok(Some(BookSyncReport {
        book_id,
        title: current.get("title"),
        volume: candidate.volume_number,
        fields_updated,
        fields_skipped,
    }))
}

/// Fetch and merge all approved external books of a series, then apply the
/// merged candidate to every matched local book.
pub(crate) async fn apply_books(
    pool: &PgPool,
    series_id: Uuid,
) -> Result<Vec<BookSyncReport>, sqlx::Error> {
    let rows = shared_sync::fetch_approved_external_books(pool, series_id).await?;
    let mut reports = Vec::new();

    for (book_id, candidate) in shared_sync::merge_book_rows(&rows) {
        if let Some(report) = apply_book(pool, book_id, &candidate).await? {
            reports.push(report);
        }
    }

    Ok(reports)
}

/// Clear external series fields when the last approved link disappears.
pub(crate) async fn clear_series_metadata(
    pool: &PgPool,
    series_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(sqlx::AssertSqlSafe(series_clear_sql()))
        .bind(series_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(provider: &str, is_primary: bool, metadata_json: Value) -> LinkMetadata {
        LinkMetadata {
            id: Uuid::new_v4(),
            is_primary,
            provider: provider.to_string(),
            external_id: "ext".to_string(),
            metadata_json,
            total_volumes_external: None,
        }
    }

    #[test]
    fn merge_series_fields_primary_wins_and_secondary_fills() {
        let primary = link(
            "google_books",
            true,
            serde_json::json!({
                "description": "from primary",
                "authors": ["A"],
                "start_year": 1999,
            }),
        );
        let secondary = link(
            "bdtheque",
            false,
            serde_json::json!({
                "description": "from secondary",
                "publishers": ["P"],
                "start_year": 2001,
                "cover_url": "http://cover",
            }),
        );

        let merged = merge_series_fields(&[secondary, primary]);
        assert_eq!(merged.description.as_deref(), Some("from primary"));
        assert_eq!(merged.authors, vec!["A".to_string()]);
        assert_eq!(merged.publishers, vec!["P".to_string()]);
        assert_eq!(merged.start_year, Some(1999));
        assert_eq!(merged.cover_url.as_deref(), Some("http://cover"));
    }

    #[test]
    fn merge_series_fields_genres_only_from_primary() {
        let primary = link(
            "google_books",
            true,
            serde_json::json!({"genres": ["Action"]}),
        );
        let secondary = link("bdtheque", false, serde_json::json!({"genres": ["Drame"]}));
        let merged = merge_series_fields(&[secondary, primary]);
        assert_eq!(merged.genres, vec!["Action".to_string()]);
    }

    #[test]
    fn merge_series_fields_genres_not_filled_from_secondary() {
        let primary = link("google_books", true, serde_json::json!({}));
        let secondary = link("bdtheque", false, serde_json::json!({"genres": ["Drame"]}));
        let merged = merge_series_fields(&[secondary, primary]);
        assert!(merged.genres.is_empty());
    }

    #[test]
    fn series_values_align_with_descriptor_order() {
        let fields = SeriesFields {
            description: Some("d".to_string()),
            authors: vec!["a".to_string()],
            publishers: vec!["p".to_string()],
            start_year: Some(2000),
            total_volumes: Some(3),
            status: Some("ongoing".to_string()),
            genres: vec!["g".to_string()],
            cover_url: Some("http://cover".to_string()),
        };

        let expected = vec![
            ("description", FieldValue::Text(Some("d".to_string()))),
            ("authors", FieldValue::Texts(vec!["a".to_string()])),
            ("publishers", FieldValue::Texts(vec!["p".to_string()])),
            ("start_year", FieldValue::Int(Some(2000))),
            ("total_volumes", FieldValue::Int(Some(3))),
            ("status", FieldValue::Text(Some("ongoing".to_string()))),
            ("genres", FieldValue::Texts(vec!["g".to_string()])),
            (
                "cover_url",
                FieldValue::Text(Some("http://cover".to_string())),
            ),
        ];

        let keys: Vec<&str> = SERIES_FIELDS
            .iter()
            .map(|descriptor| descriptor.key)
            .collect();
        let expected_keys: Vec<&str> = expected.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, expected_keys);

        let values: Vec<FieldValue> = expected.into_iter().map(|(_, value)| value).collect();
        assert_eq!(fields.values(), values);
    }

    #[test]
    fn is_field_locked_accepts_only_json_booleans() {
        let locked = serde_json::json!({
            "description": true,
            "authors": "true",
            "status": 1,
            "cover_url": false,
        });
        assert!(is_field_locked(&locked, "description"));
        assert!(!is_field_locked(&locked, "authors"));
        assert!(!is_field_locked(&locked, "status"));
        assert!(!is_field_locked(&locked, "cover_url"));
        assert!(!is_field_locked(&locked, "missing"));
    }

    #[test]
    fn sanitize_series_locks_keeps_known_true_booleans_only() {
        let sanitized = sanitize_series_locks(Some(&serde_json::json!({
            "description": true,
            "authors": true,
            "publishers": false,
            "genres": true,
            "title": true,
            "series": true,
            "status": "true",
            "cover_url": 1,
            "unknown": true,
        })));

        assert_eq!(
            sanitized,
            serde_json::json!({"description": true, "authors": true}),
            "only known lockable keys explicitly set to true survive"
        );
    }

    #[test]
    fn sanitize_book_locks_drops_stale_ui_keys_and_non_boolean_values() {
        let sanitized = sanitize_book_locks(Some(&serde_json::json!({
            "summary": true,
            "language": true,
            "isbn": "true",
            "publish_date": 1,
            "authors": false,
            "title": true,
            "series": true,
            "volume": true,
        })));

        assert_eq!(
            sanitized,
            serde_json::json!({"summary": true, "language": true})
        );
    }

    #[test]
    fn sanitize_locks_treats_missing_or_non_object_payloads_as_empty() {
        assert_eq!(sanitize_series_locks(None), serde_json::json!({}));
        assert_eq!(sanitize_book_locks(None), serde_json::json!({}));
        assert_eq!(
            sanitize_series_locks(Some(&serde_json::json!(["description"]))),
            serde_json::json!({})
        );
        assert_eq!(
            sanitize_book_locks(Some(&serde_json::json!("summary"))),
            serde_json::json!({})
        );
    }

    #[test]
    fn catalog_lists_lockable_and_clearable_fields_per_target() {
        let catalog = catalog();

        let series: Vec<(&str, bool, bool)> = catalog
            .series
            .iter()
            .map(|field| (field.key.as_str(), field.lockable, field.clearable))
            .collect();
        assert_eq!(
            series,
            vec![
                ("description", true, true),
                ("authors", true, true),
                ("publishers", true, true),
                ("start_year", true, true),
                ("total_volumes", true, true),
                ("status", true, true),
                ("genres", false, false),
                ("cover_url", true, true),
            ]
        );

        let books: Vec<(&str, bool, bool)> = catalog
            .books
            .iter()
            .map(|field| (field.key.as_str(), field.lockable, field.clearable))
            .collect();
        assert_eq!(
            books,
            vec![
                ("summary", true, false),
                ("isbn", true, false),
                ("publish_date", true, false),
                ("language", true, false),
                ("authors", true, false),
            ]
        );
    }

    #[test]
    fn series_sql_is_lock_aware_for_cover_url_and_seed_only_genres() {
        let sql = series_upsert_sql();
        assert!(sql.contains("(series.locked_fields->>'cover_url')::boolean IS TRUE"));
        assert!(sql.contains("WHEN array_length(series.genres, 1) > 0 THEN series.genres"));
        assert!(!sql.contains("(series.locked_fields->>'genres')::boolean"));
    }

    #[test]
    fn clear_sql_clears_publishers_and_skips_genres() {
        let sql = series_clear_sql();
        assert!(sql.contains("publishers = CASE WHEN"));
        assert!(sql.contains("cover_url = CASE WHEN"));
        assert!(!sql.contains("genres ="));
    }

    #[test]
    fn book_sql_derives_author_under_authors_lock() {
        let sql = book_update_sql();
        assert!(sql.contains("(locked_fields->>'authors')::boolean IS TRUE"));
        assert!(sql.contains("$7[1]"));
    }

    /// Migration 0143: BDphile's masculine statuses are mapped like the
    /// feminine ones seeded by 0038.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn masculine_bdphile_statuses_are_normalized(pool: sqlx::PgPool) {
        for (raw, expected) in [
            ("Terminé", "ended"),
            ("Annulé", "cancelled"),
            ("Abandonné", "cancelled"),
            ("Suspendu", "hiatus"),
            ("Terminée", "ended"),
            ("En cours", "ongoing"),
        ] {
            assert_eq!(normalize_series_status(&pool, raw).await, expected, "{raw}");
        }
    }
}

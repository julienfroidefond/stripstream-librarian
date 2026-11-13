//! Decide, without I/O, what a scan does with a file that is already in the database.
//!
//! Both discovery paths (changed directory, unchanged directory) observe the file the same
//! way, call [`reconcile`] once, and hand the result to [`super::writer::ScanWriter::apply`].

use chrono::{DateTime, Utc};
use parsers::{BookFormat, ParsedMetadata, VolumeType};
use uuid::Uuid;

use super::ExistingFile;

/// What the scan read from disk for one file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileStat {
    pub format: BookFormat,
    pub size_bytes: i64,
    pub mtime: DateTime<Utc>,
    pub fingerprint: String,
}

/// Whether the file content changed since the last scan, judged by its fingerprint.
#[derive(Debug, Clone, PartialEq)]
pub enum Observation {
    Unchanged,
    Changed(FileStat),
}

impl Observation {
    /// Compare a fresh stat with the stored fingerprint.
    pub fn of(existing: &ExistingFile, stat: FileStat) -> Self {
        if stat.fingerprint == existing.fingerprint {
            Self::Unchanged
        } else {
            Self::Changed(stat)
        }
    }
}

/// Filename-derived fields to rewrite on a book whose content did not change.
#[derive(Debug, Clone, PartialEq)]
pub struct Correction {
    pub book_id: Uuid,
    pub previous_title: String,
    pub previous_volume: Option<i32>,
    pub title: String,
    pub volume: Option<i32>,
    pub volume_type: VolumeType,
    /// Series name to resolve; `None` keeps the current `series_id`.
    pub series: Option<String>,
}

/// A changed file: rewrite the book and file rows and send the book back to analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct Reindex {
    pub book_id: Uuid,
    pub file_id: Uuid,
    pub title: String,
    pub volume: Option<i32>,
    pub volume_type: VolumeType,
    /// Series name to resolve; `None` detaches the book from its series.
    pub series: Option<String>,
    pub file: FileStat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reconciliation {
    Noop,
    Correct(Correction),
    Reindex(Reindex),
}

/// Reconcile a known file with what the scan observed.
///
/// `parsed.series` must already carry the series rename mapping. A changed fingerprint always
/// re-indexes; an unchanged one only corrects title, volume or volume type when they drifted.
pub fn reconcile(
    existing: &ExistingFile,
    parsed: &ParsedMetadata,
    obs: Observation,
) -> Reconciliation {
    match obs {
        Observation::Changed(file) => Reconciliation::Reindex(Reindex {
            book_id: existing.book_id,
            file_id: existing.file_id,
            title: parsed.title.clone(),
            volume: parsed.volume,
            volume_type: parsed.volume_type,
            series: parsed.series.clone(),
            file,
        }),
        Observation::Unchanged => {
            let drifted = existing.title != parsed.title
                || existing.volume != parsed.volume
                || existing.volume_type != parsed.volume_type.as_str();
            if !drifted {
                return Reconciliation::Noop;
            }
            Reconciliation::Correct(Correction {
                book_id: existing.book_id,
                previous_title: existing.title.clone(),
                previous_volume: existing.volume,
                title: parsed.title.clone(),
                volume: parsed.volume,
                volume_type: parsed.volume_type,
                series: parsed.series.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn existing(title: &str, volume: Option<i32>, volume_type: &str) -> ExistingFile {
        ExistingFile {
            file_id: Uuid::from_u128(1),
            book_id: Uuid::from_u128(2),
            fingerprint: "fp_old".to_string(),
            title: title.to_string(),
            volume,
            volume_type: volume_type.to_string(),
            series_id: None,
        }
    }

    fn parsed(title: &str, volume: Option<i32>, volume_type: VolumeType) -> ParsedMetadata {
        ParsedMetadata {
            title: title.to_string(),
            series: Some("Series".to_string()),
            volume,
            volume_type,
            page_count: None,
        }
    }

    fn stat(fingerprint: &str) -> FileStat {
        FileStat {
            format: BookFormat::Cbz,
            size_bytes: 2048,
            mtime: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            fingerprint: fingerprint.to_string(),
        }
    }

    #[test]
    fn observation_compares_fingerprints() {
        let file = existing("T", None, "regular");
        assert_eq!(
            Observation::of(&file, stat("fp_old")),
            Observation::Unchanged
        );
        assert_eq!(
            Observation::of(&file, stat("fp_new")),
            Observation::Changed(stat("fp_new"))
        );
    }

    #[test]
    fn reconcile_table() {
        use VolumeType::*;

        // (case, db (title, volume, type), parsed (title, volume, type), changed?, expected)
        #[derive(Debug, PartialEq)]
        enum Want {
            Noop,
            Correct,
            Reindex,
        }
        type Db<'a> = (&'a str, Option<i32>, &'a str);
        type Fs<'a> = (&'a str, Option<i32>, VolumeType);
        type Case<'a> = (&'a str, Db<'a>, Fs<'a>, bool, Want);
        let cases: Vec<Case> = vec![
            (
                "all fields match",
                ("Series - T05", Some(5), "regular"),
                ("Series - T05", Some(5), Regular),
                false,
                Want::Noop,
            ),
            (
                "file renamed",
                ("Old Title", None, "regular"),
                ("Series - T05", Some(5), Regular),
                false,
                Want::Correct,
            ),
            (
                "volume missing in db",
                ("Kaiju no8 - Tome 1", None, "regular"),
                ("Kaiju no8 - Tome 1", Some(1), Regular),
                false,
                Want::Correct,
            ),
            (
                "volume type drifted",
                ("Series HS", None, "regular"),
                ("Series HS", None, Hs),
                false,
                Want::Correct,
            ),
            (
                "content changed, same name",
                ("Series - T05", Some(5), "regular"),
                ("Series - T05", Some(5), Regular),
                true,
                Want::Reindex,
            ),
            (
                "content changed and renamed",
                ("Old", None, "regular"),
                ("Series - T05", Some(5), Regular),
                true,
                Want::Reindex,
            ),
        ];

        for (case, db, fs, changed, want) in cases {
            let file = existing(db.0, db.1, db.2);
            let meta = parsed(fs.0, fs.1, fs.2);
            let obs = if changed {
                Observation::Changed(stat("fp_new"))
            } else {
                Observation::Unchanged
            };
            let got = match reconcile(&file, &meta, obs) {
                Reconciliation::Noop => Want::Noop,
                Reconciliation::Correct(c) => {
                    assert_eq!(c.book_id, file.book_id, "{case}");
                    assert_eq!(
                        (c.previous_title.as_str(), c.previous_volume),
                        (db.0, db.1),
                        "{case}"
                    );
                    assert_eq!((c.title.as_str(), c.volume, c.volume_type), fs, "{case}");
                    assert_eq!(c.series.as_deref(), Some("Series"), "{case}");
                    Want::Correct
                }
                Reconciliation::Reindex(r) => {
                    assert_eq!(
                        (r.book_id, r.file_id),
                        (file.book_id, file.file_id),
                        "{case}"
                    );
                    assert_eq!((r.title.as_str(), r.volume, r.volume_type), fs, "{case}");
                    assert_eq!(r.file, stat("fp_new"), "{case}");
                    Want::Reindex
                }
            };
            assert_eq!(got, want, "{case}");
        }
    }

    #[test]
    fn correction_without_series_keeps_none() {
        let file = existing("Old", None, "regular");
        let mut meta = parsed("New", None, VolumeType::Regular);
        meta.series = None;
        match reconcile(&file, &meta, Observation::Unchanged) {
            Reconciliation::Correct(c) => assert_eq!(c.series, None),
            other => panic!("expected a correction, got {other:?}"),
        }
    }
}

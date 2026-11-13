//! On-disk thumbnail layout: `<directory>/<book_id>.<webp|jpg|png>`, plus the
//! transient `<book_id>.raw` page extracted by the indexer before encoding.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::warn;
use uuid::Uuid;

use crate::encode::{encode, EncodedThumbnail, ThumbnailExt};
use crate::settings::ThumbnailSettings;

const RAW_EXT: &str = "raw";

/// The thumbnail store of one configuration. All writes are atomic
/// (temporary file + rename), so a reader never sees a partial image.
#[derive(Debug, Clone)]
pub struct ThumbnailStore {
    settings: ThumbnailSettings,
}

impl ThumbnailStore {
    pub fn new(settings: ThumbnailSettings) -> Self {
        Self { settings }
    }

    pub fn settings(&self) -> &ThumbnailSettings {
        &self.settings
    }

    pub fn directory(&self) -> &Path {
        &self.settings.directory
    }

    /// Encode a raw page image with the configured size, quality and format.
    /// CPU bound: call it from a blocking task.
    pub fn encode(&self, source: &[u8]) -> Result<EncodedThumbnail> {
        encode(source, &self.settings)
    }

    /// Path of the thumbnail of `book_id` with extension `ext`.
    pub fn path_for(&self, book_id: Uuid, ext: ThumbnailExt) -> PathBuf {
        self.directory().join(format!("{book_id}.{}", ext.as_str()))
    }

    /// Path of the raw (not yet encoded) page of `book_id`.
    pub fn raw_path_for(&self, book_id: Uuid) -> PathBuf {
        self.directory().join(format!("{book_id}.{RAW_EXT}"))
    }

    /// Book a store file belongs to: `<uuid>.<webp|jpg|png|raw>`. Any other
    /// file name (temporary files included) returns `None`.
    pub fn book_id_of(path: &Path) -> Option<Uuid> {
        let stem = path.file_stem()?.to_str()?;
        let ext = path.extension()?.to_str()?;
        if ext != RAW_EXT && ThumbnailExt::from_file_ext(ext).is_none() {
            return None;
        }
        Uuid::parse_str(stem).ok()
    }

    /// Atomically publish `thumbnail` as the thumbnail of `book_id`, then drop
    /// the thumbnails of the same book left in another format. Returns the
    /// published path.
    pub fn publish(&self, book_id: Uuid, thumbnail: &EncodedThumbnail) -> io::Result<PathBuf> {
        let path = self.path_for(book_id, thumbnail.ext);
        write_atomic(&path, &thumbnail.bytes)?;
        for ext in ThumbnailExt::ALL {
            if ext != thumbnail.ext {
                remove_if_present(&self.path_for(book_id, ext));
            }
        }
        Ok(path)
    }

    /// Atomically store the raw page of `book_id`, to be encoded later.
    pub fn stage_raw(&self, book_id: Uuid, bytes: &[u8]) -> io::Result<PathBuf> {
        let path = self.raw_path_for(book_id);
        write_atomic(&path, bytes)?;
        Ok(path)
    }

    /// Encode the staged raw page of `book_id`, publish it and delete the raw
    /// file. Returns the published path.
    pub fn publish_raw(&self, book_id: Uuid, raw_path: &Path) -> Result<PathBuf> {
        let raw = std::fs::read(raw_path)
            .map_err(|e| anyhow::anyhow!("failed to read raw image {}: {e}", raw_path.display()))?;
        let thumbnail = self.encode(&raw)?;
        let path = self.publish(book_id, &thumbnail)?;
        remove_if_present(raw_path);
        Ok(path)
    }

    /// Delete every file of `book_id` (all formats and the raw page). Returns
    /// the number of thumbnails removed, raw pages excluded.
    pub fn purge(&self, book_id: Uuid) -> usize {
        remove_if_present(&self.raw_path_for(book_id));
        ThumbnailExt::ALL
            .into_iter()
            .filter(|ext| remove_if_present(&self.path_for(book_id, *ext)))
            .count()
    }

    /// Delete the store files whose book is not in `known_books`. Returns the
    /// number of files removed.
    pub fn purge_orphans(&self, known_books: &HashSet<Uuid>) -> usize {
        let Ok(entries) = std::fs::read_dir(self.directory()) else {
            return 0;
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                Self::book_id_of(path).is_some_and(|book_id| !known_books.contains(&book_id))
            })
            .filter(|path| remove_if_present(path))
            .count()
    }
}

/// Write `bytes` to `path` through a temporary sibling and a rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("thumbnail path has no parent directory"))?;
    std::fs::create_dir_all(directory)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("invalid thumbnail file name"))?;
    let temporary = directory.join(format!(".{file_name}-{}.tmp", Uuid::new_v4()));
    std::fs::write(&temporary, bytes)?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

/// Remove `path` when it exists. Returns whether a file was removed; failures
/// other than "not found" are logged.
fn remove_if_present(path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            warn!(target: "thumbnail", "[THUMBNAIL] cannot delete {}: {error}", path.display());
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::test_images::png_bytes;
    use crate::settings::ThumbnailFormat;

    fn store(dir: &Path, format: ThumbnailFormat) -> ThumbnailStore {
        ThumbnailSettings {
            directory: dir.to_path_buf(),
            format,
            ..ThumbnailSettings::default()
        }
        .store()
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn publish_writes_atomically_into_a_created_directory() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("nested/thumbs");
        let store = store(&dir, ThumbnailFormat::Webp);
        let book = Uuid::new_v4();
        let thumbnail = EncodedThumbnail {
            bytes: b"bytes".to_vec(),
            ext: ThumbnailExt::Webp,
        };

        let path = store.publish(book, &thumbnail).unwrap();

        assert_eq!(path, dir.join(format!("{book}.webp")));
        assert_eq!(std::fs::read(&path).unwrap(), b"bytes");
        assert_eq!(
            files(&dir),
            vec![format!("{book}.webp")],
            "no temp file left"
        );
    }

    #[test]
    fn publish_replaces_a_thumbnail_stored_in_another_format() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), ThumbnailFormat::Webp);
        let book = Uuid::new_v4();
        std::fs::write(store.path_for(book, ThumbnailExt::Webp), b"old").unwrap();

        let thumbnail = EncodedThumbnail {
            bytes: b"new".to_vec(),
            ext: ThumbnailExt::Jpg,
        };
        store.publish(book, &thumbnail).unwrap();

        assert_eq!(files(dir.path()), vec![format!("{book}.jpg")]);
    }

    #[test]
    fn staged_raw_page_is_encoded_published_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), ThumbnailFormat::Png);
        let book = Uuid::new_v4();

        let raw = store.stage_raw(book, &png_bytes(600, 800)).unwrap();
        assert_eq!(raw, dir.path().join(format!("{book}.raw")));

        let published = store.publish_raw(book, &raw).unwrap();
        assert_eq!(published, dir.path().join(format!("{book}.png")));
        assert!(!raw.exists());
        let (w, h) = image::GenericImageView::dimensions(
            &image::load_from_memory(&std::fs::read(&published).unwrap()).unwrap(),
        );
        assert_eq!((w, h), (300, 400));
    }

    #[test]
    fn purge_removes_every_format_and_the_raw_page() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), ThumbnailFormat::Webp);
        let book = Uuid::new_v4();
        let other = Uuid::new_v4();
        for name in [
            format!("{book}.webp"),
            format!("{book}.jpg"),
            format!("{book}.raw"),
            format!("{other}.webp"),
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }

        assert_eq!(store.purge(book), 2, "raw pages are not counted");
        assert_eq!(files(dir.path()), vec![format!("{other}.webp")]);
        assert_eq!(store.purge(book), 0);
    }

    #[test]
    fn purge_orphans_keeps_known_books_and_foreign_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), ThumbnailFormat::Webp);
        let kept = Uuid::new_v4();
        let orphan = Uuid::new_v4();
        for name in [
            format!("{kept}.webp"),
            format!("{orphan}.png"),
            format!("{orphan}.raw"),
            "README.txt".to_string(),
            "not-a-uuid.webp".to_string(),
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }

        let removed = store.purge_orphans(&HashSet::from([kept]));

        assert_eq!(removed, 2);
        let mut expected = vec![
            "README.txt".to_string(),
            format!("{kept}.webp"),
            "not-a-uuid.webp".to_string(),
        ];
        expected.sort();
        assert_eq!(files(dir.path()), expected);
    }

    #[test]
    fn purge_orphans_tolerates_a_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir.path().join("absent"), ThumbnailFormat::Webp);
        assert_eq!(store.purge_orphans(&HashSet::new()), 0);
    }

    #[test]
    fn book_id_of_parses_store_file_names_only() {
        let book = Uuid::new_v4();
        for ext in ["webp", "jpg", "png", "raw"] {
            let path = PathBuf::from(format!("/t/{book}.{ext}"));
            assert_eq!(ThumbnailStore::book_id_of(&path), Some(book));
        }
        assert_eq!(
            ThumbnailStore::book_id_of(Path::new(&format!("/t/{book}.gif"))),
            None
        );
        assert_eq!(
            ThumbnailStore::book_id_of(Path::new(&format!("/t/.{book}.webp-x.tmp"))),
            None
        );
        assert_eq!(ThumbnailStore::book_id_of(Path::new("/t/abc.webp")), None);
    }

    #[test]
    fn path_for_uses_the_configured_directory() {
        let store = store(Path::new("/srv/thumbs"), ThumbnailFormat::Webp);
        let book = Uuid::new_v4();
        assert_eq!(
            store.path_for(book, ThumbnailExt::Png),
            PathBuf::from(format!("/srv/thumbs/{book}.png"))
        );
        assert_eq!(
            store.raw_path_for(book),
            PathBuf::from(format!("/srv/thumbs/{book}.raw"))
        );
    }
}

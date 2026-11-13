//! The single thumbnail store shared by the API and the indexer.
//!
//! - [`ThumbnailSettings::load`] is the only way to read the thumbnail
//!   configuration (one precedence: the `thumbnail` row of `app_settings`,
//!   field by field, over the built-in defaults — no environment variable).
//! - [`ThumbnailStore`] owns the on-disk layout (`<directory>/<book_id>.<ext>`),
//!   the encoding, the atomic publication and the purges.
//! - [`fast_jpeg_decode`] is the shared DCT-scaled JPEG decoder used by both the
//!   thumbnail encoder and the API page renderer.

mod decode;
mod encode;
mod settings;
mod store;

pub use decode::{fast_jpeg_decode, FastJpeg};
pub use encode::{EncodedThumbnail, ThumbnailExt};
pub use settings::{ThumbnailFormat, ThumbnailSettings, DEFAULT_DIRECTORY, SETTINGS_KEY};
pub use store::ThumbnailStore;

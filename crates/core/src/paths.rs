use std::fmt;
use std::path::{Path, PathBuf};

/// Prefix used in the database for every path inside the mounted libraries root.
pub const LIBRARIES_DB_ROOT: &str = "/libraries";

/// The local directory the `/libraries` database prefix is mounted on.
///
/// Construct it once from configuration ([`LibraryRoot::from_env`]) and pass it
/// to the code that translates database paths to host paths. Keeping the root in
/// a value avoids reading `LIBRARIES_ROOT_PATH` on every conversion and makes the
/// translation direction explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRoot {
    local_root: PathBuf,
}

/// A filesystem path that belongs to a library, carrying both of its forms.
///
/// Build it with [`LibraryRoot::from_db_path`], [`LibraryRoot::from_local_path`]
/// or [`LibraryRoot::resolve`]; never by joining strings by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryPath {
    db_path: String,
    local_path: String,
}

/// Why a path could not be interpreted as a [`LibraryPath`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryPathError {
    /// Library paths must be absolute; relative input is ambiguous.
    RelativePath { path: String },
    /// The path does not start with [`LIBRARIES_DB_ROOT`].
    OutsideDatabaseRoot { path: String },
    /// The path is not inside the configured local libraries root.
    OutsideLocalRoot { path: String, local_root: String },
}

impl fmt::Display for LibraryPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativePath { path } => {
                write!(f, "library path must be absolute: {path}")
            }
            Self::OutsideDatabaseRoot { path } => {
                write!(
                    f,
                    "path is not inside the libraries database root {LIBRARIES_DB_ROOT}: {path}"
                )
            }
            Self::OutsideLocalRoot { path, local_root } => {
                write!(
                    f,
                    "path is not inside the local libraries root {local_root}: {path}"
                )
            }
        }
    }
}

impl std::error::Error for LibraryPathError {}

impl LibraryRoot {
    /// Build a root from the host directory that holds the libraries.
    ///
    /// Trailing separators and `.` components are dropped so a root configured
    /// as `/host/books/` behaves exactly like `/host/books`. An empty value
    /// falls back to the database root, where translation is the identity.
    pub fn new(local_root: impl Into<PathBuf>) -> Self {
        let local_root = local_root.into();
        let local_root = if local_root.as_os_str().is_empty() {
            PathBuf::from(LIBRARIES_DB_ROOT)
        } else {
            local_root.components().collect()
        };
        Self { local_root }
    }

    /// Read the root from `LIBRARIES_ROOT_PATH`, defaulting to `/libraries`.
    ///
    /// This is the only place the environment variable is read. Services build
    /// their root at startup and pass it down.
    pub fn from_env() -> Self {
        std::env::var_os("LIBRARIES_ROOT_PATH")
            .map(PathBuf::from)
            .map(Self::new)
            .unwrap_or_else(|| Self::new(LIBRARIES_DB_ROOT))
    }

    /// The host directory libraries are mounted on.
    pub fn local_root(&self) -> &Path {
        &self.local_root
    }

    /// Whether `path` is a database path under [`LIBRARIES_DB_ROOT`].
    ///
    /// Component-aware: `/libraries-other/x` is not a database path.
    pub fn is_database_path(path: &str) -> bool {
        Path::new(path).strip_prefix(LIBRARIES_DB_ROOT).is_ok()
    }

    /// Translate a database path (`/libraries/...`) to its local and database forms.
    pub fn from_db_path(&self, db_path: &str) -> Result<LibraryPath, LibraryPathError> {
        if !Path::new(db_path).is_absolute() {
            return Err(LibraryPathError::RelativePath {
                path: db_path.to_string(),
            });
        }

        let relative = Path::new(db_path)
            .strip_prefix(LIBRARIES_DB_ROOT)
            .map_err(|_| LibraryPathError::OutsideDatabaseRoot {
                path: db_path.to_string(),
            })?;
        let local_path = if relative.as_os_str().is_empty() {
            self.local_root.clone()
        } else {
            self.local_root.join(relative)
        };

        Ok(LibraryPath {
            db_path: db_path.to_string(),
            local_path: local_path.to_string_lossy().into_owned(),
        })
    }

    /// Translate a local path inside the mounted root back to a database path.
    pub fn from_local_path(&self, local_path: &str) -> Result<LibraryPath, LibraryPathError> {
        if !Path::new(local_path).is_absolute() {
            return Err(LibraryPathError::RelativePath {
                path: local_path.to_string(),
            });
        }

        let relative = Path::new(local_path)
            .strip_prefix(&self.local_root)
            .map_err(|_| LibraryPathError::OutsideLocalRoot {
                path: local_path.to_string(),
                local_root: self.local_root.to_string_lossy().into_owned(),
            })?;
        let db_path = if relative.as_os_str().is_empty() {
            PathBuf::from(LIBRARIES_DB_ROOT)
        } else {
            Path::new(LIBRARIES_DB_ROOT).join(relative)
        };

        Ok(LibraryPath {
            db_path: db_path.to_string_lossy().into_owned(),
            local_path: local_path.to_string(),
        })
    }

    /// Resolve a path that may already be a database path, a path inside the
    /// mounted root, or an absolute host path (legacy library roots, files
    /// indexed before the mount was configured).
    ///
    /// Relative input is rejected: it cannot be classified safely.
    pub fn resolve(&self, path: &str) -> Result<LibraryPath, LibraryPathError> {
        if !Path::new(path).is_absolute() {
            return Err(LibraryPathError::RelativePath {
                path: path.to_string(),
            });
        }

        if Self::is_database_path(path) {
            return self.from_db_path(path);
        }

        if Path::new(path).strip_prefix(&self.local_root).is_ok() {
            return self.from_local_path(path);
        }

        // An absolute path outside both roots is already a local filesystem
        // path (for example a library rooted at `/Volumes/Comics`). It carries
        // no database mapping, so both forms are the same string.
        Ok(LibraryPath {
            db_path: path.to_string(),
            local_path: path.to_string(),
        })
    }
}

impl LibraryPath {
    /// The host filesystem path.
    pub fn to_local(&self) -> &str {
        &self.local_path
    }

    /// The database path (`/libraries/...` or an absolute legacy host path).
    pub fn to_db(&self) -> &str {
        &self.db_path
    }
}

/// Resolve `.`/`..` components lexically (without touching the filesystem), so
/// path-traversal can be detected from the string alone.
pub fn normalize_lexically(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;

    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> LibraryRoot {
        LibraryRoot::new("/home/user/books")
    }

    #[test]
    fn maps_database_path_to_local() {
        let path = root()
            .from_db_path("/libraries/BD/les geants/06. yatho.cbz")
            .unwrap();
        assert_eq!(
            path.to_local(),
            "/home/user/books/BD/les geants/06. yatho.cbz"
        );
        assert_eq!(path.to_db(), "/libraries/BD/les geants/06. yatho.cbz");
    }

    #[test]
    fn maps_local_path_to_database() {
        let path = root()
            .from_local_path("/home/user/books/BD/test.cbz")
            .unwrap();
        assert_eq!(path.to_db(), "/libraries/BD/test.cbz");
        assert_eq!(path.to_local(), "/home/user/books/BD/test.cbz");
    }

    #[test]
    fn round_trips_both_directions() {
        let db = "/libraries/manga/naruto/vol01.cbz";
        let local = root().from_db_path(db).unwrap().to_local().to_string();
        assert_eq!(local, "/home/user/books/manga/naruto/vol01.cbz");
        assert_eq!(root().from_local_path(&local).unwrap().to_db(), db);
    }

    #[test]
    fn exact_roots_map_to_each_other() {
        assert_eq!(
            root().from_db_path("/libraries").unwrap().to_local(),
            "/home/user/books"
        );
        assert_eq!(
            root().from_local_path("/home/user/books").unwrap().to_db(),
            "/libraries"
        );
    }

    #[test]
    fn database_root_requires_a_component_boundary() {
        let err = root()
            .from_db_path("/libraries-other/book.cbz")
            .unwrap_err();
        assert!(matches!(err, LibraryPathError::OutsideDatabaseRoot { .. }));
    }

    #[test]
    fn local_root_requires_a_component_boundary() {
        let err = root()
            .from_local_path("/home/user/books-other/book.cbz")
            .unwrap_err();
        assert!(matches!(err, LibraryPathError::OutsideLocalRoot { .. }));
    }

    #[test]
    fn resolve_keeps_absolute_paths_outside_the_mount() {
        let path = root().resolve("/Volumes/Comics/book.cbz").unwrap();
        assert_eq!(path.to_local(), "/Volumes/Comics/book.cbz");
        assert_eq!(path.to_db(), "/Volumes/Comics/book.cbz");
    }

    #[test]
    fn resolve_prefers_the_database_prefix() {
        let path = root().resolve("/libraries/BD/test.cbz").unwrap();
        assert_eq!(path.to_local(), "/home/user/books/BD/test.cbz");
    }

    #[test]
    fn resolve_maps_paths_inside_the_local_root_back_to_the_database() {
        let path = root().resolve("/home/user/books/BD/test.cbz").unwrap();
        assert_eq!(path.to_db(), "/libraries/BD/test.cbz");
    }

    #[test]
    fn rejects_relative_paths() {
        assert!(matches!(
            root().resolve("BD/test.cbz").unwrap_err(),
            LibraryPathError::RelativePath { .. }
        ));
        assert!(matches!(
            root().from_db_path("libraries/BD/test.cbz").unwrap_err(),
            LibraryPathError::RelativePath { .. }
        ));
        assert!(matches!(
            root().from_local_path("books/BD/test.cbz").unwrap_err(),
            LibraryPathError::RelativePath { .. }
        ));
    }

    #[test]
    fn trailing_slash_on_the_root_is_normalized() {
        let root = LibraryRoot::new("/home/user/books/");
        assert_eq!(
            root.from_db_path("/libraries/BD/test.cbz")
                .unwrap()
                .to_local(),
            "/home/user/books/BD/test.cbz"
        );
        assert_eq!(
            root.from_local_path("/home/user/books/BD/test.cbz")
                .unwrap()
                .to_db(),
            "/libraries/BD/test.cbz"
        );
    }

    #[test]
    fn default_root_is_the_identity_mapping() {
        let root = LibraryRoot::new(LIBRARIES_DB_ROOT);
        assert_eq!(
            root.from_db_path("/libraries/BD/test.cbz")
                .unwrap()
                .to_local(),
            "/libraries/BD/test.cbz"
        );
        assert_eq!(
            root.from_local_path("/libraries/BD/test.cbz")
                .unwrap()
                .to_db(),
            "/libraries/BD/test.cbz"
        );
    }

    #[test]
    fn empty_root_falls_back_to_the_database_root() {
        let root = LibraryRoot::new("");
        assert_eq!(root.local_root(), Path::new(LIBRARIES_DB_ROOT));
    }

    #[test]
    fn normalize_resolves_dot_and_dotdot() {
        use std::path::{Path, PathBuf};
        assert_eq!(
            normalize_lexically(Path::new("/a/./b/../c")),
            PathBuf::from("/a/c")
        );
        assert_eq!(
            normalize_lexically(Path::new("a/../../b")),
            PathBuf::from("b")
        );
        assert_eq!(
            normalize_lexically(Path::new("./a/b/")),
            PathBuf::from("a/b")
        );
    }
}

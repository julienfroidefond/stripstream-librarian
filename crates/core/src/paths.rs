/// Remap a DB path (starting with `/libraries/`) to the local filesystem path
/// using the `LIBRARIES_ROOT_PATH` environment variable.
pub fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}

/// Convert a local filesystem path back to a DB path (starting with `/libraries/`).
pub fn unmap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with(&root) {
            return path.replacen(&root, "/libraries", 1);
        }
    }
    path.to_string()
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
    use std::sync::Mutex;

    // Serialize tests that modify env vars
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn remap_with_env_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("LIBRARIES_ROOT_PATH", "/home/user/books");
        assert_eq!(
            remap_libraries_path("/libraries/BD/les geants/06. yatho.cbz"),
            "/home/user/books/BD/les geants/06. yatho.cbz"
        );
        std::env::remove_var("LIBRARIES_ROOT_PATH");
    }

    #[test]
    fn remap_without_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("LIBRARIES_ROOT_PATH");
        assert_eq!(
            remap_libraries_path("/libraries/BD/test.cbz"),
            "/libraries/BD/test.cbz"
        );
    }

    #[test]
    fn remap_non_matching_prefix() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("LIBRARIES_ROOT_PATH", "/home/user/books");
        assert_eq!(
            remap_libraries_path("/other/path/file.cbz"),
            "/other/path/file.cbz"
        );
        std::env::remove_var("LIBRARIES_ROOT_PATH");
    }

    #[test]
    fn unmap_with_env_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("LIBRARIES_ROOT_PATH", "/home/user/books");
        assert_eq!(
            unmap_libraries_path("/home/user/books/BD/test.cbz"),
            "/libraries/BD/test.cbz"
        );
        std::env::remove_var("LIBRARIES_ROOT_PATH");
    }

    #[test]
    fn unmap_without_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("LIBRARIES_ROOT_PATH");
        assert_eq!(
            unmap_libraries_path("/home/user/books/BD/test.cbz"),
            "/home/user/books/BD/test.cbz"
        );
    }

    #[test]
    fn roundtrip() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("LIBRARIES_ROOT_PATH", "/mnt/data");
        let original = "/libraries/manga/naruto/vol01.cbz";
        let remapped = remap_libraries_path(original);
        assert_eq!(remapped, "/mnt/data/manga/naruto/vol01.cbz");
        assert_eq!(unmap_libraries_path(&remapped), original);
        std::env::remove_var("LIBRARIES_ROOT_PATH");
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

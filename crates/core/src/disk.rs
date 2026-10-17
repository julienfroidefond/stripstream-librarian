//! Filesystem capacity of the volumes the services write to.
//!
//! A full disk is the most common operational failure (thumbnail generation
//! silently stops, imports half-fail), and nothing else in the stack reports it.
//! `statvfs` is used through `libc` so the same code works on the Linux
//! containers and on a macOS development machine.

use std::ffi::CString;
use std::path::Path;

/// Capacity of one mounted filesystem, as seen from a path.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskSpace {
    /// The path that was measured (kept as given, for display).
    pub path: String,
    pub total_bytes: u64,
    /// Bytes usable by the current user (`f_bavail`), not just free blocks.
    pub free_bytes: u64,
    pub used_bytes: u64,
    /// Share of the volume in use, rounded to one decimal.
    pub used_percent: f64,
    /// `false` when the path does not exist or cannot be stat'ed.
    pub available: bool,
}

impl DiskSpace {
    /// Placeholder for a path that could not be measured.
    fn unavailable(path: &str) -> Self {
        Self {
            path: path.to_string(),
            total_bytes: 0,
            free_bytes: 0,
            used_bytes: 0,
            used_percent: 0.0,
            available: false,
        }
    }

    /// Whether the volume is close to full (less than 10 % usable).
    pub fn is_critical(&self) -> bool {
        self.available && self.total_bytes > 0 && self.used_percent >= 90.0
    }
}

/// Measure the filesystem holding `path`.
///
/// Missing paths are reported as unavailable rather than as an error: a
/// diagnostics page must render even when a mount is gone.
pub fn space(path: &str) -> DiskSpace {
    if !Path::new(path).exists() {
        return DiskSpace::unavailable(path);
    }

    let Ok(c_path) = CString::new(path) else {
        return DiskSpace::unavailable(path);
    };

    // SAFETY: `statvfs` only writes into the `statvfs` struct we pass, and the
    // C string is NUL-terminated and outlives the call.
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if rc != 0 {
        return DiskSpace::unavailable(path);
    }

    // SAFETY: `statvfs` returned 0, so the struct was initialised.
    let stats = unsafe { stats.assume_init() };

    // `f_frsize` is the real fragment size, `f_bsize` the legacy fallback; both
    // are already `u64` on the 64-bit platforms we ship (Linux, macOS).
    let block_size = if stats.f_frsize > 0 {
        stats.f_frsize
    } else {
        stats.f_bsize
    };

    // `fsblkcnt_t` is `u32` on macOS but `u64` on Linux, so the cast is required
    // on one target and unnecessary on the other.
    #[allow(clippy::unnecessary_cast)]
    let total_bytes = (stats.f_blocks as u64).saturating_mul(block_size);
    #[allow(clippy::unnecessary_cast)]
    let free_bytes = (stats.f_bavail as u64).saturating_mul(block_size);
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let used_percent = if total_bytes > 0 {
        ((used_bytes as f64 / total_bytes as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    DiskSpace {
        path: path.to_string(),
        total_bytes,
        free_bytes,
        used_bytes,
        used_percent,
        available: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_the_current_directory() {
        let disk = space(".");
        assert!(disk.available);
        assert!(disk.total_bytes > 0);
        assert!(disk.used_bytes <= disk.total_bytes);
        assert!((0.0..=100.0).contains(&disk.used_percent));
    }

    #[test]
    fn missing_path_is_reported_as_unavailable() {
        let disk = space("/this/path/does/not/exist/stripstream");
        assert!(!disk.available);
        assert_eq!(disk.total_bytes, 0);
        assert!(!disk.is_critical());
    }

    #[test]
    fn critical_requires_an_available_volume() {
        let mut disk = DiskSpace::unavailable("/missing");
        disk.used_percent = 99.0;
        assert!(!disk.is_critical());

        disk.available = true;
        disk.total_bytes = 1024;
        assert!(disk.is_critical());
    }
}

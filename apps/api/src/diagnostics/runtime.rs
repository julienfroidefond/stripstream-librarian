//! Provisioning of the API process: memory, descriptors, rendering slots.

use super::RuntimeDiagnostics;
use crate::state::AppState;

pub(super) fn runtime_diagnostics(state: &AppState) -> RuntimeDiagnostics {
    let rss_bytes = resident_set_bytes();
    let render_limit = state.page_render_limit_total;

    RuntimeDiagnostics {
        rss_bytes,
        rss_pretty: rss_bytes.map(super::format_bytes),
        open_fds: open_file_descriptors(),
        renders_in_flight: render_limit.saturating_sub(state.page_render_limit.available_permits()),
        render_limit,
    }
}

/// Peak resident set size of the process.
///
/// `getrusage` works on both Linux and macOS; only the unit differs
/// (kilobytes on Linux, bytes on macOS).
fn resident_set_bytes() -> Option<u64> {
    // SAFETY: `getrusage` writes into the struct we pass and does not retain it.
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }

    // SAFETY: `getrusage` returned 0, so the struct was initialised.
    let usage = unsafe { usage.assume_init() };
    let raw = u64::try_from(usage.ru_maxrss).ok()?;

    if cfg!(target_os = "macos") {
        Some(raw)
    } else {
        Some(raw.saturating_mul(1024))
    }
}

/// Number of open file descriptors (Linux only).
#[cfg(target_os = "linux")]
fn open_file_descriptors() -> Option<u64> {
    std::fs::read_dir("/proc/self/fd")
        .ok()
        .map(|entries| entries.count() as u64)
}

/// macOS exposes descriptors through `libproc`, which is not worth the extra
/// dependency for a diagnostics panel.
#[cfg(not(target_os = "linux"))]
fn open_file_descriptors() -> Option<u64> {
    None
}

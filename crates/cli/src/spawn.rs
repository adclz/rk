//! Locating the deployable runtime binary.
//!
//! The CLI never runs a PLC in-process: `rk test` and the runtime both spawn
//! the runtime, the same binary a plant runs. This module is how they find it.

use std::path::PathBuf;

/// The the runtime that goes with *this* `rk`.
///
/// Beside the current executable first, and only then `PATH`. A toolchain must
/// not run one version's program under another version's runtime just because
/// an older copy happens to come first in `PATH` — and during development the
/// binary next to `rk` is the one that was just rebuilt.
pub fn runtime_binary() -> PathBuf {
    let exe = if cfg!(windows) {
        "runtime.exe"
    } else {
        "runtime"
    };
    if let Ok(current) = std::env::current_exe()
        && let Some(dir) = current.parent()
    {
        let sibling = dir.join(exe);
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from(exe)
}

/// The hint that goes with "could not start it": the runtime is a separate
/// binary now, so its absence is a failure mode a user can actually hit.
pub fn missing_hint(binary: &std::path::Path, e: &std::io::Error) -> String {
    format!(
        "could not start `{}`: {e}\n       \
         The runtime is a separate binary. Build it with \
         `cargo build --bin runtime`.",
        binary.display()
    )
}

//! Locating the binaries that go with *this* `rk`.
//!
//! Beside the current executable first, and only then `PATH`. A toolchain must
//! not run one version's program under another version's binary just because
//! an older copy happens to come first in `PATH` — and during development the
//! binary next to `rk` is the one that was just rebuilt.

use std::path::PathBuf;

/// The language server that goes with *this* `rk`. A tool launches it;
/// nothing in a toolchain builds it.
pub fn lsp_binary() -> PathBuf {
    sibling_or_path(if cfg!(windows) {
        "vscode-lsp-server.exe"
    } else {
        "vscode-lsp-server"
    })
}

/// `exe` beside the current executable when it is there, else bare, for
/// `PATH` to resolve.
pub fn sibling_or_path(exe: &str) -> PathBuf {
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

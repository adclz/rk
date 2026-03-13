//! Embeds the standard library `.st` files into the binary at compile time
//! and extracts them to `$HOME/.rk_std/` on first use (or when the version changes).

use std::fs;
use std::path::PathBuf;

/// Version marker — bump this (or tie to package version) to force re-extraction.
const STDLIB_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Each embedded stdlib file: (filename, content).
const EMBEDDED_FILES: &[(&str, &str)] = &[
    ("Bistable.st", include_str!("../../../stdlib/Bistable.st")),
    ("Bits.st", include_str!("../../../stdlib/Bits.st")),
    ("Convert.st", include_str!("../../../stdlib/Convert.st")),
    ("Counters.st", include_str!("../../../stdlib/Counters.st")),
    ("Edge.st", include_str!("../../../stdlib/Edge.st")),
    ("Math.st", include_str!("../../../stdlib/Math.st")),
    ("Memory.st", include_str!("../../../stdlib/Memory.st")),
    ("Timers.st", include_str!("../../../stdlib/Timers.st")),
    ("Selection.st", include_str!("../../../stdlib/Selection.st")),
    ("Unit.st", include_str!("../../../stdlib/Unit.st")),
];

/// Returns the stdlib extraction directory: `$HOME/.rk_std/`.
fn stdlib_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".rk_std"))
}

/// Extracts the embedded stdlib to `$HOME/.rk_std/` if needed.
///
/// Extraction happens when:
/// - The directory doesn't exist, or
/// - The `.version` marker doesn't match the current version.
///
/// Returns the path to the extracted stdlib directory, or `None` on failure.
pub fn ensure_stdlib_extracted() -> Option<PathBuf> {
    let dir = stdlib_dir()?;
    let version_file = dir.join(".version");

    let needs_extraction = if dir.exists() {
        // Check version marker
        fs::read_to_string(&version_file)
            .map(|v| v.trim() != STDLIB_VERSION)
            .unwrap_or(true)
    } else {
        true
    };

    if needs_extraction {
        // Remove old version if present
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }

        fs::create_dir_all(&dir).ok()?;

        for (filename, content) in EMBEDDED_FILES {
            let path = dir.join(filename);
            if fs::write(&path, content).is_err() {
                eprintln!("failed to extract stdlib file: {}", filename);
                let _ = fs::remove_dir_all(&dir);
                return None;
            }
        }

        // Write version marker
        let _ = fs::write(&version_file, STDLIB_VERSION);
    }

    Some(dir)
}

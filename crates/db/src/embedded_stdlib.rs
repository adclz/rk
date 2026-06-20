//! Embeds the standard library `.st` files into the binary at compile time
//! and extracts them to `$HOME/.rk_std/` on first use (or when the version changes).

use std::fs;
use std::path::PathBuf;

/// Cache marker written to `$HOME/.rk_std/.version`: the crate version plus a
/// hash of the embedded file *contents*. Tying it to content (not just the
/// crate version) forces re-extraction whenever a stdlib `.st` file changes —
/// otherwise `~/.rk_std` silently goes stale across stdlib edits that don't bump
/// the crate version, which is how `debug` ends up loading a *deprecated* stdlib.
fn stdlib_marker() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for &(name, content) in EMBEDDED_FILES {
        name.hash(&mut hasher);
        content.hash(&mut hasher);
    }
    format!("{}-{:016x}", env!("CARGO_PKG_VERSION"), hasher.finish())
}

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
    ("Strings.st", include_str!("../../../stdlib/Strings.st")),
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
    let marker = stdlib_marker();

    let needs_extraction = if dir.exists() {
        // Re-extract when the embedded content (its hash) changed.
        fs::read_to_string(&version_file)
            .map(|v| v.trim() != marker)
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

        // Write the content marker.
        let _ = fs::write(&version_file, &marker);
    }

    Some(dir)
}

#[cfg(test)]
mod tests {
    use super::EMBEDDED_FILES;
    use std::collections::HashSet;
    use std::path::Path;

    /// Guards the bug that shipped an *incomplete* stdlib: every `stdlib/*.st`
    /// must be in [`EMBEDDED_FILES`], or it isn't extracted to `~/.rk_std` and
    /// references to it (e.g. `Std.Strings.CONCAT`) resolve to `Never` in any
    /// release build — or any debug build run outside the repo root.
    #[test]
    fn every_stdlib_file_is_embedded() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stdlib");
        let embedded: HashSet<&str> = EMBEDDED_FILES.iter().map(|(n, _)| *n).collect();
        for entry in std::fs::read_dir(&dir).expect("read stdlib dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("st") {
                let name = path.file_name().unwrap().to_str().unwrap();
                assert!(
                    embedded.contains(name),
                    "stdlib/{name} is not in EMBEDDED_FILES — add it, or `rk` ships an incomplete stdlib"
                );
            }
        }
    }
}

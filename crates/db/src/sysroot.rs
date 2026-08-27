//! Where the standard library lives, found the way a compiler finds its
//! sysroot: relative to the executable that is asking.
//!
//! Installing the library is packaging's job — unpacking an archive, or a CI
//! job assembling one. Finding it is the compiler's. The two must not be
//! confused: a compiler that writes a library into `$HOME` on first use is an
//! installer wearing a compiler's name, and it fails on read-only roots, in
//! containers, and for a second user on the same machine.
//!
//! Two layouts satisfy the probe:
//!
//! ```text
//! <prefix>/bin/rk                    <repo>/target/debug/rk
//! <prefix>/lib/rk/std/config.toml    <repo>/stdlib/config.toml
//! ```
//!
//! The development tree is one of them rather than a `cfg!(debug_assertions)`
//! branch, so the checkout satisfies the shipped layout and the tests
//! exercise the code path that ships.
//!
//! This module only ANSWERS. Nothing here reads the environment or decides
//! precedence; [`crate::loader::resolve_library_path`] owns that order and
//! consults the probe last.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// What declares a directory usable by `rk` — a library or a workspace, the
/// same file either way. Already required of the standard library, which is a
/// project like any other (`stdlib/config.toml`).
pub const CONFIG_FILE: &str = "config.toml";

/// The installed layout, relative to a directory on the search path.
const SYSROOT_SUFFIX: &[&str] = &["lib", "rk", "std"];

/// The development layout: the library beside the checkout it belongs to.
const DEV_SUFFIX: &[&str] = &["stdlib"];

/// How far above the executable's own directory to look.
///
/// Three ancestors is the least that reaches every layout in use:
///
/// | executable                            | ancestor |
/// |---------------------------------------|----------|
/// | `<prefix>/bin/rk`                     | 1        |
/// | `target/debug/rk`                     | 2        |
/// | `target/<triple>/release/rk`          | 3        |
/// | `target/debug/deps/<test binary>`     | 3        |
/// | `vscode/server/bin/vscode-lsp-server` | 3        |
///
/// It is also a stopping rule: an unbounded walk reaches `$HOME` and `/`,
/// where a stray project would silently become everyone's standard library.
const MAX_ANCESTORS: usize = 3;

/// Where a library directory came from, for `rk env` and for the message that
/// names the paths when none was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryOrigin {
    /// `RK_STDLIB_PATH` in the process environment.
    Env,
    /// `RK_STDLIB_PATH` in a `.env` at the workspace root.
    Dotenv,
    /// Found beside the executable, in the installed layout.
    Sysroot,
    /// Found beside the executable, in a development checkout.
    DevTree,
}

impl LibraryOrigin {
    /// The spelling used in user-facing output.
    pub fn as_str(self) -> &'static str {
        match self {
            LibraryOrigin::Env => "env",
            LibraryOrigin::Dotenv => "dotenv",
            LibraryOrigin::Sysroot => "sysroot",
            LibraryOrigin::DevTree => "dev-tree",
        }
    }
}

/// The library directory belonging to `exe`, or `None`; nearest wins.
pub fn probe_from(exe: &Path) -> Option<(LibraryOrigin, PathBuf)> {
    for dir in search_roots(exe) {
        for (origin, suffix) in [
            (LibraryOrigin::Sysroot, SYSROOT_SUFFIX),
            (LibraryOrigin::DevTree, DEV_SUFFIX),
        ] {
            let candidate = join_all(&dir, suffix);
            if is_library(&candidate) {
                return Some((origin, candidate));
            }
        }
    }
    None
}

/// [`probe_from`] for the running executable, cached: the answer cannot
/// change while the process lives.
pub fn probe() -> Option<(LibraryOrigin, PathBuf)> {
    static PROBED: OnceLock<Option<(LibraryOrigin, PathBuf)>> = OnceLock::new();
    PROBED
        .get_or_init(|| probe_from(&std::env::current_exe().ok()?))
        .clone()
}

/// [`probed_paths`] for the running executable.
pub fn probed() -> Vec<PathBuf> {
    std::env::current_exe()
        .map(|exe| probed_paths(&exe))
        .unwrap_or_default()
}

/// Every directory [`probe_from`] would look in, in order, for the
/// diagnostic when nothing was found.
pub fn probed_paths(exe: &Path) -> Vec<PathBuf> {
    search_roots(exe)
        .into_iter()
        .flat_map(|dir| {
            [SYSROOT_SUFFIX, DEV_SUFFIX]
                .into_iter()
                .map(move |suffix| join_all(&dir, suffix))
        })
        .collect()
}

/// The directories to search, nearest first.
///
/// Both the executable's own path and its canonical one are walked: a
/// packager may symlink `bin/rk` from elsewhere, and the library sits beside
/// the real file, not beside the link. Go removed its `GOROOT_FINAL` knob and
/// told packagers to symlink for exactly this reason.
fn search_roots(exe: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let canonical = exe.canonicalize().ok();
    for path in [Some(exe.to_path_buf()), canonical].into_iter().flatten() {
        let Some(parent) = path.parent() else {
            continue;
        };
        for ancestor in parent.ancestors().take(MAX_ANCESTORS + 1) {
            let ancestor = ancestor.to_path_buf();
            if !roots.contains(&ancestor) {
                roots.push(ancestor);
            }
        }
    }
    roots
}

/// Whether a directory declares itself usable, by holding a [`CONFIG_FILE`].
///
/// The probe searches directories nobody pointed at, so it cannot take the
/// first path that merely exists: an empty `lib/rk/std` left behind by a
/// half-finished copy would become a standard library with nothing in it.
/// Requiring the declaration a library already carries keeps the walk from
/// inventing a second notion of what a library is.
///
/// A path named through `RK_STDLIB_PATH` skips this: that one is an explicit
/// choice and is taken as given.
fn is_library(dir: &Path) -> bool {
    dir.join(CONFIG_FILE).is_file()
}

fn join_all(base: &Path, suffix: &[&str]) -> PathBuf {
    let mut path = base.to_path_buf();
    for part in suffix {
        path.push(part);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Build a tree under `root`: create each directory, and write a project
    /// declaration into every path naming one.
    fn tree(root: &Path, paths: &[&str]) {
        for rel in paths {
            let path = root.join(rel);
            if rel.ends_with(CONFIG_FILE) {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, "[project]\nname = \"std\"\nversion = \"0.1\"\n").unwrap();
            } else {
                fs::create_dir_all(&path).unwrap();
            }
        }
    }

    #[test]
    fn installed_layout_is_found_from_bin() {
        let t = tempfile::tempdir().unwrap();
        tree(t.path(), &["bin", "lib/rk/std/config.toml"]);
        let found = probe_from(&t.path().join("bin/rk")).unwrap();
        assert_eq!(found.0, LibraryOrigin::Sysroot);
        assert_eq!(found.1, t.path().join("lib/rk/std"));
    }

    /// An unpacked archive with the executable at its root, rather than in a
    /// `bin/` — the same probe covers it at one level less.
    #[test]
    fn flat_layout_is_found() {
        let t = tempfile::tempdir().unwrap();
        tree(t.path(), &["lib/rk/std/config.toml"]);
        let found = probe_from(&t.path().join("rk")).unwrap();
        assert_eq!(found.0, LibraryOrigin::Sysroot);
    }

    /// Each executable path a cargo build produces, against the checkout's
    /// own `stdlib/`: the depths `MAX_ANCESTORS` exists to cover.
    #[test]
    fn every_build_layout_reaches_the_dev_tree() {
        for exe in [
            "target/debug/rk",
            "target/release/rk",
            "target/x86_64-unknown-linux-gnu/release/rk",
            "target/debug/deps/db-0123456789abcdef",
            "vscode/server/bin/vscode-lsp-server",
        ] {
            let t = tempfile::tempdir().unwrap();
            tree(t.path(), &["stdlib/config.toml"]);
            let found = probe_from(&t.path().join(exe))
                .unwrap_or_else(|| panic!("{exe} must reach the checkout's stdlib"));
            assert_eq!(found.0, LibraryOrigin::DevTree, "{exe}");
            assert_eq!(found.1, t.path().join("stdlib"), "{exe}");
        }
    }

    /// A directory that declares nothing is not a library, even when it
    /// holds sources.
    #[test]
    fn a_directory_without_a_declaration_is_not_a_library() {
        let t = tempfile::tempdir().unwrap();
        tree(t.path(), &["stdlib", "lib/rk/std"]);
        fs::write(t.path().join("stdlib/Math.st"), "FUNCTION f : INT\nEND_FUNCTION\n").unwrap();
        assert!(probe_from(&t.path().join("target/debug/rk")).is_none());
    }

    #[test]
    fn nothing_at_all_is_none() {
        let t = tempfile::tempdir().unwrap();
        assert!(probe_from(&t.path().join("bin/rk")).is_none());
    }

    /// Nearest wins, so a library beside the executable is never shadowed by
    /// one further up the tree.
    #[test]
    fn the_nearest_library_wins() {
        let t = tempfile::tempdir().unwrap();
        tree(
            t.path(),
            &["stdlib/config.toml", "target/debug/lib/rk/std/config.toml"],
        );
        let found = probe_from(&t.path().join("target/debug/rk")).unwrap();
        assert_eq!(found.0, LibraryOrigin::Sysroot);
        assert_eq!(found.1, t.path().join("target/debug/lib/rk/std"));
    }

    /// The stopping rule: one level past the bound is not reached, or a stray
    /// marker in `$HOME` would become everyone's standard library.
    #[test]
    fn the_walk_stops_at_the_bound() {
        let t = tempfile::tempdir().unwrap();
        tree(t.path(), &["stdlib/config.toml"]);
        // parent + 3 ancestors reaches the root; one deeper does not.
        assert!(probe_from(&t.path().join("a/b/c/rk")).is_some());
        assert!(probe_from(&t.path().join("a/b/c/d/rk")).is_none());
    }

    /// A packager may symlink the executable; the library sits beside the
    /// real file, not beside the link.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_executable_finds_the_real_tree() {
        let t = tempfile::tempdir().unwrap();
        tree(t.path(), &["real/bin", "real/lib/rk/std/config.toml", "link"]);
        let real = t.path().join("real/bin/rk");
        fs::write(&real, "").unwrap();
        let link = t.path().join("link/rk");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let found = probe_from(&link).unwrap();
        assert_eq!(found.0, LibraryOrigin::Sysroot);
        assert_eq!(
            found.1.canonicalize().unwrap(),
            t.path().join("real/lib/rk/std").canonicalize().unwrap()
        );
    }

    /// The probe reports what it looked at, so "no library" can be told apart
    /// from "looked in the wrong place".
    #[test]
    fn probed_paths_name_both_layouts_at_every_level() {
        let paths = probed_paths(Path::new("/opt/rk/bin/rk"));
        assert!(paths.contains(&PathBuf::from("/opt/rk/lib/rk/std")));
        assert!(paths.contains(&PathBuf::from("/opt/rk/stdlib")));
        assert!(paths.contains(&PathBuf::from("/opt/rk/bin/lib/rk/std")));
    }

    /// This test binary must find the checkout's own `stdlib/`, pinning the
    /// depth bound.
    #[test]
    fn this_test_binary_finds_the_checkouts_stdlib() {
        let exe = std::env::current_exe().expect("a test binary has a path");
        let (origin, dir) = probe_from(&exe)
            .unwrap_or_else(|| panic!("no library found from {}", exe.display()));
        assert_eq!(origin, LibraryOrigin::DevTree);
        assert!(dir.join("Math.st").is_file(), "{} holds the stdlib", dir.display());
    }
}

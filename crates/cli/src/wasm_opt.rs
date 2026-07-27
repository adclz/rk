//! Locating `wasm-opt`, Binaryen's optimizer.
//!
//! There is no usable Rust binding. The `wasm-opt` crate wraps Binaryen **116**
//! and was last released in March 2024; upstream Binaryen is at 131. That gap
//! matters concretely rather than cosmetically: `try_table` — the instruction
//! this compiler emits for `RAISE` and for the stdlib's assertions — did not
//! exist in 116, so that binding cannot even READ a module from any realistic
//! workspace. Linking it also meant compiling 178 C++ files on every clean
//! build to produce something that could not do the job.
//!
//! So the optimizer is an external binary, the way `wasm-pack` and the rest of
//! the wasm toolchain treat it:
//!
//! 1. a `wasm-opt` already on PATH — whatever the user or CI installed wins;
//! 2. otherwise a pinned prebuilt release, downloaded once into a cache
//!    directory and checksum-verified;
//! 3. otherwise nothing, and `-O` degrades to an unoptimized (still correct)
//!    build with a warning.
//!
//! Step 2 can be refused: set `RK_NO_DOWNLOAD=1` for an air-gapped or
//! reproducible environment and the search stops at step 1.

use std::path::{Path, PathBuf};

use crate::ui;

/// The Binaryen release fetched when none is installed, pinned so a build
/// does not change behaviour when upstream publishes; CI installs the same
/// version.
const BINARYEN_VERSION: &str = "131";

/// Environment variable that refuses the download step.
const NO_DOWNLOAD_ENV: &str = "RK_NO_DOWNLOAD";

/// Locate a usable `wasm-opt`, downloading the pinned release if needed;
/// `None` when none can be obtained.
pub fn find(verbose: bool) -> Option<PathBuf> {
    if let Some(path) = on_path() {
        if verbose {
            ui::detail(format!("    using wasm-opt from PATH ({})", path.display()));
        }
        return Some(path);
    }

    let cached = cache_dir()?.join(format!("binaryen-version_{BINARYEN_VERSION}"));
    let binary = cached.join("bin").join(exe_name());
    if binary.is_file() {
        if verbose {
            ui::detail(format!("    using cached wasm-opt ({})", binary.display()));
        }
        return Some(binary);
    }

    if std::env::var_os(NO_DOWNLOAD_ENV).is_some() {
        ui::warn(format!(
            "no `wasm-opt` on PATH and {NO_DOWNLOAD_ENV} is set, so none will be downloaded"
        ));
        return None;
    }

    match download(&cached) {
        Ok(()) if binary.is_file() => Some(binary),
        Ok(()) => {
            ui::warn(format!(
                "downloaded Binaryen {BINARYEN_VERSION} but found no `{}` inside it",
                exe_name()
            ));
            None
        }
        Err(e) => {
            ui::warn(format!(
                "could not obtain wasm-opt: {e}. Install Binaryen {BINARYEN_VERSION} or newer \
                 and put `wasm-opt` on PATH."
            ));
            None
        }
    }
}

/// A `wasm-opt` already installed, if any. Whatever the user has wins: it is
/// likely newer than the pin, and CI installs one deliberately.
fn on_path() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(exe_name()))
        .find(|candidate| candidate.is_file())
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "wasm-opt.exe" } else { "wasm-opt" }
}

/// Where a downloaded Binaryen lives: the user's cache directory, so it
/// survives between builds and is never written into the workspace.
fn cache_dir() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("rk").join("binaryen"))
}

/// The release asset for this platform, or `None`. Binaryen says `aarch64`
/// for Linux and `arm64` for macOS and Windows, for the same architecture.
fn asset_name() -> Option<String> {
    let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-linux",
        ("linux", "aarch64") => "aarch64-linux",
        ("macos", "x86_64") => "x86_64-macos",
        ("macos", "aarch64") => "arm64-macos",
        ("windows", "x86_64") => "x86_64-windows",
        ("windows", "aarch64") => "arm64-windows",
        _ => return None,
    };
    Some(format!(
        "binaryen-version_{BINARYEN_VERSION}-{platform}.tar.gz"
    ))
}

/// Fetch, verify and unpack the pinned release into `dest`'s parent. The
/// checksum is not optional: this downloads an executable and runs it.
fn download(dest: &Path) -> Result<(), String> {
    let asset = asset_name().ok_or_else(|| {
        format!(
            "Binaryen publishes no build for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let base = format!(
        "https://github.com/WebAssembly/binaryen/releases/download/version_{BINARYEN_VERSION}"
    );
    let parent = dest
        .parent()
        .ok_or_else(|| "cache path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;

    ui::detail(format!(
        "    Downloading Binaryen {BINARYEN_VERSION} (one time, into {})",
        parent.display()
    ));

    let archive = get(&format!("{base}/{asset}"))?;
    let expected = String::from_utf8(get(&format!("{base}/{asset}.sha256"))?)
        .map_err(|_| "checksum file is not text".to_string())?;
    // The file is `<hex>  <filename>`; take the hash.
    let expected = expected
        .split_whitespace()
        .next()
        .ok_or_else(|| "checksum file is empty".to_string())?
        .to_ascii_lowercase();

    let actual = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&archive);
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    if actual != expected {
        return Err(format!(
            "checksum mismatch for {asset} (expected {expected}, got {actual})"
        ));
    }

    // Unpack into a temporary sibling and rename, so a concurrent or
    // interrupted build never observes a half-extracted toolchain.
    let staging = tempfile::Builder::new()
        .prefix(".binaryen-staging-")
        .tempdir_in(parent)
        .map_err(|e| format!("creating a staging dir: {e}"))?;
    tar::Archive::new(flate2::read::GzDecoder::new(&archive[..]))
        .unpack(staging.path())
        .map_err(|e| format!("unpacking {asset}: {e}"))?;

    let unpacked = staging
        .path()
        .join(format!("binaryen-version_{BINARYEN_VERSION}"));
    if dest.exists() {
        // Another build won the race; theirs is as good as ours.
        return Ok(());
    }
    std::fs::rename(&unpacked, dest).map_err(|e| format!("installing into {}: {e}", dest.display()))
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut body = ureq::get(url)
        .call()
        .map_err(|e| format!("fetching {url}: {e}"))?
        .into_body();
    let mut bytes = Vec::new();
    std::io::copy(&mut body.as_reader(), &mut bytes)
        .map_err(|e| format!("reading {url}: {e}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every platform the project builds for must map to a real Binaryen asset
    /// — a typo here is only discovered by someone on that platform.
    #[test]
    fn the_asset_name_matches_binaryen_s_publishing_scheme() {
        // The host platform must be one Binaryen ships, or `-O` can never work
        // here without a manual install.
        let asset = asset_name().expect("Binaryen should publish a build for the host");
        assert!(asset.starts_with(&format!("binaryen-version_{BINARYEN_VERSION}-")));
        assert!(asset.ends_with(".tar.gz"));
    }

    /// The cache must live outside the workspace: a toolchain downloaded into
    /// the project would end up in diffs, in artifacts, and in every clone.
    #[test]
    fn the_cache_is_not_in_the_workspace() {
        let dir = cache_dir().expect("a cache directory");
        let cwd = std::env::current_dir().expect("cwd");
        assert!(
            !dir.starts_with(&cwd),
            "the Binaryen cache must not sit inside the workspace, got {}",
            dir.display()
        );
    }
}

// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The global cache: one per user, shared by every workspace, a folder per
//! hash that is written once and never written again.

use db::config_file::CONFIG_FILE;
use db::dependencies::{ContentHash, Remote, cached_urls, packages_dir};
use fetch::{fetch, fetch_remote};

use super::{CONFIG, archive, served};

/// What the cache has is not written again, and its files are not to
/// be written at all.
#[test]
fn a_library_in_the_cache_is_left_as_it_is() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[("config.toml", CONFIG), ("Axis.st", "one")]),
    );
    let first = fetch(&cache, &url).unwrap();
    let file = first.dir.join("Axis.st");
    assert!(std::fs::metadata(&file).unwrap().permissions().readonly());
    let written = std::fs::metadata(&file).unwrap().modified().unwrap();

    let again = fetch(&cache, &url).unwrap();
    assert_eq!(again.dir, first.dir);
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        written
    );
    let urls = std::fs::read_to_string(cached_urls(&cache, &first.hash)).unwrap();
    assert_eq!(urls, format!("{url}\n"));
    // Nothing is left of the folder it was unpacked in.
    let left: Vec<_> = std::fs::read_dir(packages_dir(&cache))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert_eq!(left, [] as [String; 0]);
}

/// The hash written beside a URL is the one of what that URL serves.
#[test]
fn another_content_than_the_hash_says_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[("config.toml", CONFIG)]),
    );
    let actual = fetch(&cache, &url).unwrap().hash;
    let expected = ContentHash::from_digest(&[7; 32]);
    let remote = |hash| Remote {
        name: "drives",
        url: &url,
        hash,
    };
    assert_eq!(fetch_remote(&cache, &remote(&actual)), Ok(()));
    assert_eq!(
        fetch_remote(&cache, &remote(&expected))
            .unwrap_err()
            .message,
        format!(
            "hash mismatch: config.toml declares {expected} but the fetched library has {actual}"
        )
    );
}

/// The cache is the user's, not a workspace's: a library fetched for one
/// workspace is read by another from the same folder, with nothing
/// downloaded, even once the archive is gone.
#[test]
fn two_workspaces_share_one_copy() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let cache = root.join("cache");
    let archive_file = root.join("drives.tar.gz");
    let url = served(
        &root,
        "drives.tar.gz",
        &archive(&[("config.toml", CONFIG), ("Axis.st", "one")]),
    );
    let fetched = fetch(&cache, &url).unwrap();
    std::fs::remove_file(&archive_file).unwrap();

    for name in ["plant", "line"] {
        let workspace = root.join(name);
        std::fs::create_dir_all(&workspace).unwrap();
        let source = format!(
            "[project]\nname = \"{name}\"\nversion = \"1\"\n\n[dependencies]\ndrives = {{ url = \"{url}\", hash = \"{}\" }}\n",
            fetched.hash
        );
        std::fs::write(workspace.join(CONFIG_FILE), &source).unwrap();
        let config = db::config_file::parse_config(&source).unwrap();
        let resolution = db::dependencies::resolve(&workspace, &config, None, Some(&cache), None);
        assert!(
            resolution.problems.is_empty(),
            "{name}: {:?}",
            resolution.problems
        );
        assert_eq!(
            resolution.libraries[0].dir,
            fetched.dir.canonicalize().unwrap()
        );
    }
}

/// The cache is in the user's cache folder, and `RK_CACHE_DIR` puts it
/// elsewhere.
#[test]
fn the_cache_is_the_users_unless_rk_cache_dir_says() {
    let t = tempfile::tempdir().unwrap();
    // SAFETY: nextest runs each test in a process of its own.
    unsafe { std::env::set_var(db::cache::CACHE_DIR_ENV, t.path()) };
    assert_eq!(db::cache::root(), Some(t.path().to_path_buf()));
    unsafe { std::env::remove_var(db::cache::CACHE_DIR_ENV) };
    let default = db::cache::root().expect("a cache folder");
    assert!(default.ends_with("rk"), "{}", default.display());
}

/// Builds that fetch the same library at the same moment all get it, and
/// the cache ends with one copy of it and nothing else.
#[test]
fn builds_that_fetch_at_once_end_with_one_copy() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[
            ("config.toml", CONFIG),
            ("src/Axis.st", "one"),
            ("src/Ramp.st", "two"),
        ]),
    );
    let fetched: Vec<_> = std::thread::scope(|scope| {
        let builds: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| fetch(&cache, &url).unwrap()))
            .collect();
        builds
            .into_iter()
            .map(|build| build.join().unwrap())
            .collect()
    });
    let first = &fetched[0];
    for other in &fetched[1..] {
        assert_eq!(other.hash, first.hash);
        assert_eq!(other.dir, first.dir);
    }
    let mut entries: Vec<String> = std::fs::read_dir(packages_dir(&cache))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    let hex = first.hash.hex();
    assert_eq!(entries, [hex.to_string(), format!("{hex}.urls")]);
    let urls = std::fs::read_to_string(cached_urls(&cache, &first.hash)).unwrap();
    assert!(urls.lines().all(|line| line == url), "{urls}");
}

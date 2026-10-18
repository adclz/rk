// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What differs from one system to another: where the cache is, how its
//! files are kept from being written, links and file URLs. And what is
//! refused on every system because one of them reads it otherwise: a
//! `path` written for one system, a name another cannot take.
//!
//! The tests marked for one system run on it in CI, which tests on Linux,
//! macOS and Windows.

use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", windows))]
use std::path::PathBuf;

use db::cache::{Entry, StoreError};
use db::config_file::parse_config;
use db::dependencies::{ContentHash, packages_dir, resolve};
use fetch::fetch;

use super::{CONFIG, archive, served};

/// The cache sits where the system keeps the caches of its users, unless
/// `RK_CACHE_DIR` says otherwise. On Linux that is `XDG_CACHE_HOME` when it
/// is a full path, and `~/.cache` otherwise.
#[cfg(target_os = "linux")]
#[test]
fn the_cache_is_in_xdg_cache_home_on_linux() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    // SAFETY: nextest runs each test in a process of its own.
    unsafe {
        std::env::remove_var(db::cache::CACHE_DIR_ENV);
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CACHE_HOME", t.path().join("xdg"));
    }
    assert_eq!(db::cache::root(), Some(t.path().join("xdg/rk")));

    unsafe { std::env::set_var("XDG_CACHE_HOME", "relative/xdg") };
    assert_eq!(db::cache::root(), Some(home.join(".cache/rk")));

    unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    assert_eq!(db::cache::root(), Some(home.join(".cache/rk")));
}

/// On macOS the caches of a user are in `~/Library/Caches`.
#[cfg(target_os = "macos")]
#[test]
fn the_cache_is_in_library_caches_on_macos() {
    let t = tempfile::tempdir().unwrap();
    // SAFETY: nextest runs each test in a process of its own.
    unsafe {
        std::env::remove_var(db::cache::CACHE_DIR_ENV);
        std::env::set_var("HOME", t.path());
    }
    assert_eq!(
        db::cache::root(),
        Some(PathBuf::from(t.path()).join("Library/Caches/rk"))
    );
}

/// On Windows the caches of a user are in the local application data,
/// `%LOCALAPPDATA%`.
#[cfg(windows)]
#[test]
fn the_cache_is_in_local_app_data_on_windows() {
    // SAFETY: nextest runs each test in a process of its own.
    unsafe { std::env::remove_var(db::cache::CACHE_DIR_ENV) };
    let local = std::env::var_os("LOCALAPPDATA").expect("Windows names LOCALAPPDATA");
    assert_eq!(db::cache::root(), Some(PathBuf::from(local).join("rk")));
}

/// A library of a file and a program.
fn sources_and_a_program() -> BTreeMap<String, Entry> {
    BTreeMap::from([
        ("Axis.st".to_string(), Entry::File(b"x".to_vec())),
        ("bin/run".to_string(), Entry::File(b"#!/bin/sh\n".to_vec())),
    ])
}

/// On Unix a file of the cache can be read by everyone and written by no
/// one, and a program can be run as well.
#[cfg(unix)]
#[test]
fn a_cached_file_is_read_only_by_its_mode_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let entries = sources_and_a_program();
    let dir = db::cache::store(t.path(), &ContentHash::of(&entries), &entries).unwrap();
    let mode = |path: &str| {
        std::fs::metadata(dir.join(path))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("Axis.st"), 0o444);
    assert_eq!(mode("bin/run"), 0o555);
}

/// On Windows a file of the cache has the read-only attribute, and the
/// cache can still delete it.
#[cfg(windows)]
#[test]
fn a_cached_file_is_read_only_by_its_attribute_on_windows() {
    let t = tempfile::tempdir().unwrap();
    let entries = sources_and_a_program();
    let dir = db::cache::store(t.path(), &ContentHash::of(&entries), &entries).unwrap();
    for path in ["Axis.st", "bin/run"] {
        let metadata = std::fs::metadata(dir.join(path)).unwrap();
        assert!(metadata.permissions().readonly(), "{path}");
    }
    db::cache::remove(&dir).unwrap();
    assert!(!dir.exists());
}

/// On Windows a link may need a right the user does not have. The link is
/// then reported, and the library is not kept without it.
#[cfg(windows)]
#[test]
fn a_link_is_made_or_reported_on_windows() {
    let t = tempfile::tempdir().unwrap();
    let entries = BTreeMap::from([
        ("Axis.st".to_string(), Entry::File(b"x".to_vec())),
        ("alias.st".to_string(), Entry::Link("Axis.st".to_string())),
    ]);
    match db::cache::store(t.path(), &ContentHash::of(&entries), &entries) {
        Ok(dir) => assert_eq!(std::fs::read(dir.join("alias.st")).unwrap(), b"x"),
        Err(error) => {
            assert!(matches!(error, StoreError::Link { .. }), "{error}");
            assert_eq!(
                std::fs::read_dir(packages_dir(t.path())).unwrap().count(),
                0
            );
        }
    }
}

/// On Windows a file URL names a drive: `file:///C:/...`.
#[cfg(windows)]
#[test]
fn a_file_url_names_its_drive_on_windows() {
    let t = tempfile::tempdir().unwrap();
    let url = served(
        t.path(),
        "drives.tar.gz",
        &archive(&[("config.toml", CONFIG)]),
    );
    let drive = url.strip_prefix("file:///").unwrap().as_bytes();
    assert!(drive[0].is_ascii_alphabetic() && drive[1] == b':', "{url}");
    let fetched = fetch(&t.path().join("cache"), &url).unwrap();
    assert_eq!(fetched.name, "drives");
}

/// What could not be written leaves nothing in the cache. Some file
/// systems do not delete a read-only file, so the files written before are
/// made writable again first.
#[test]
fn a_library_that_cannot_be_written_leaves_nothing() {
    let t = tempfile::tempdir().unwrap();
    // `a` is written as a file, and then `a/b` needs it to be a folder.
    let entries = BTreeMap::from([
        ("a".to_string(), Entry::File(b"x".to_vec())),
        ("a/b".to_string(), Entry::File(b"y".to_vec())),
    ]);
    let error = db::cache::store(t.path(), &ContentHash::of(&entries), &entries).unwrap_err();
    assert!(matches!(error, StoreError::Write { .. }), "{error}");
    assert_eq!(
        std::fs::read_dir(packages_dir(t.path())).unwrap().count(),
        0
    );
}

/// A `path` is read the same on every system. One that a system reads from
/// a root or a drive is refused on all of them, and so is a `\`, which is a
/// separator on one system and a letter on another.
#[test]
fn a_path_is_read_the_same_on_every_system() {
    let t = tempfile::tempdir().unwrap();
    let ws = t.path().canonicalize().unwrap();
    std::fs::create_dir(ws.join("shared")).unwrap();
    let cases = [
        ("/opt/shared", "'/opt/shared' is an absolute path"),
        ("C:/shared", "'C:/shared' is an absolute path"),
        ("c:shared", "'c:shared' is an absolute path"),
        ("C:\\\\shared", "'C:\\shared' is an absolute path"),
        ("\\\\shared", "'\\shared' is an absolute path"),
        (
            "\\\\\\\\server\\\\shared",
            "'\\\\server\\shared' is an absolute path",
        ),
        (
            "..\\\\ws\\\\shared",
            "'..\\ws\\shared' has `\\` between its folders",
        ),
    ];
    for (path, message) in cases {
        let config = parse_config(&format!(
            "[project]\nname = \"ws\"\nversion = \"1\"\n\n[dependencies]\nshared = {{ path = \"{path}\" }}\n"
        ))
        .unwrap();
        let resolution = resolve(&ws, &config, None, None, None);
        let problems: Vec<_> = resolution
            .problems
            .iter()
            .map(|problem| problem.message.as_str())
            .collect();
        assert_eq!(problems, [message], "{path}");
    }
    let config = parse_config(
        "[project]\nname = \"ws\"\nversion = \"1\"\n\n[dependencies]\nshared = { path = \"..\\\\ws\\\\shared\" }\n",
    )
    .unwrap();
    let problem = &resolve(&ws, &config, None, None, None).problems[0];
    assert_eq!(problem.help.as_deref(), Some("write '../ws/shared'"));
}

/// A name some system cannot give a file is refused on every system: one
/// it keeps for a device, one with a character it refuses, and one that
/// ends with a dot or a space, which it drops.
#[test]
fn a_name_some_system_cannot_take_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let names = [
        "aux.st",
        "CON",
        "nul.txt",
        "Com1.st",
        "lpt9",
        "src/prn.st",
        "a<b.st",
        "a>b.st",
        "a\"b.st",
        "a|b.st",
        "a?b.st",
        "a*b.st",
        "Axis.st.",
        "Axis.st ",
    ];
    for (index, name) in names.iter().enumerate() {
        let url = served(
            t.path(),
            &format!("{index}.tar.gz"),
            &archive(&[("config.toml", CONFIG), (name, "x")]),
        );
        assert_eq!(
            fetch(&cache, &url).unwrap_err().to_string(),
            format!(
                "error: unable to unpack tarball\n    note: file '{name}' has a name that not every system accepts\n"
            ),
        );
    }
    // A name that only starts like one of them is a name like any other.
    let url = served(
        t.path(),
        "taken.tar.gz",
        &archive(&[
            ("config.toml", CONFIG),
            ("console.st", "x"),
            ("auxiliary/Com10.st", "x"),
            (".hidden", "x"),
        ]),
    );
    fetch(&cache, &url).unwrap();
}

// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The standard library built into the compiler: what it holds, its copy
//! in the cache, and the workspaces that read it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::PositionEncodingKind;
use db::cache::Entry;
use db::dependencies::{cached_dir, packages_dir};
use db::stdlib::{Standard, bundle, install, locate};
use db::{RootDatabase, WorkspaceDataBase};

use super::{Expected, expected_hash, package_files};

/// The checkout's `stdlib/`.
fn stdlib_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib")
}

/// The names in the cache's folder of libraries.
fn cached_names(cache: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(packages_dir(cache))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// `file`, made writable again, as someone would to change it.
fn writable(file: PathBuf) -> PathBuf {
    let mut permissions = std::fs::metadata(&file).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&file, permissions).unwrap();
    file
}

/// The standard library built in, written in a cache of the test's own.
fn built_in(cache: &Path) {
    // SAFETY: nextest runs each test in a process of its own.
    unsafe {
        std::env::remove_var(db::stdlib::PATH_ENV);
        std::env::set_var(db::cache::CACHE_DIR_ENV, cache);
    }
}

/// The workspace at `root`, opened as the editor opens it, and the
/// notices it gave.
fn open(root: &Path) -> (RootDatabase, Vec<String>) {
    let mut db = RootDatabase::default();
    let mut notices = vec![];
    db::loader::open_workspace(
        &mut db,
        Some(root),
        PositionEncodingKind::UTF8,
        db::loader::rk_parser,
        None,
        &mut vec![],
        &mut notices,
    );
    (db, notices.iter().map(ToString::to_string).collect())
}

/// The compiler holds the files of `stdlib/` as they are: every file
/// there but the hidden ones and the build folder, its sources, its
/// `config.toml` and its `LICENSE` among them.
#[test]
fn the_compiler_holds_the_files_of_stdlib() {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, Entry>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "rk_build" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                let relative = relative.to_string_lossy().replace('\\', "/");
                files.insert(relative, Entry::File(std::fs::read(&path).unwrap()));
            }
        }
    }
    let dir = stdlib_folder();
    let mut on_disk = BTreeMap::new();
    walk(&dir, &dir, &mut on_disk);
    let names = |entries: &BTreeMap<String, Entry>| entries.keys().cloned().collect::<Vec<_>>();
    assert_eq!(names(&bundle().entries), names(&on_disk));
    assert!(bundle().entries == on_disk, "a file differs from stdlib/");
    for name in ["config.toml", "LICENSE", "Math.st"] {
        assert!(bundle().entries.contains_key(name), "{name}");
    }
    assert_eq!(bundle().name, "stdlib");
}

/// The hash of the standard library is the one of its files, as for any
/// library.
#[test]
fn the_standard_library_has_the_hash_of_its_files() {
    let entries: Vec<(&str, Expected<'_>)> = bundle()
        .entries
        .iter()
        .map(|(path, entry)| match entry {
            Entry::File(content) => (path.as_str(), Expected::File(content)),
            Entry::Link(link) => (path.as_str(), Expected::Link(link)),
        })
        .collect();
    assert_eq!(bundle().hash.to_string(), expected_hash(&entries));
}

/// The standard library is written in the cache under its hash, with
/// nothing beside it, and its files are not to be written.
#[test]
fn the_standard_library_is_written_in_the_cache_under_its_hash() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let dir = install(&cache).unwrap();

    assert_eq!(dir, cached_dir(&cache, &bundle().hash));
    assert_eq!(db::cache::read(&dir).unwrap(), bundle().entries);
    for file in package_files(&dir) {
        let metadata = std::fs::metadata(dir.join(&file)).unwrap();
        assert!(metadata.permissions().readonly(), "{file}");
    }
    assert_eq!(cached_names(&cache), [bundle().hash.hex()]);
}

/// The copy in the cache is hashed at each start: a file changed, added
/// or removed there is put back as the compiler holds it.
#[test]
fn a_copy_that_differs_is_written_again() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    type Change = fn(&Path);
    let changes: [(&str, Change); 3] = [
        ("changed", |dir| {
            let math = writable(dir.join("Math.st"));
            std::fs::write(math, "FUNCTION changed : INT\nEND_FUNCTION\n").unwrap();
        }),
        ("added", |dir| {
            std::fs::write(dir.join("Extra.st"), "FUNCTION extra : INT\nEND_FUNCTION\n").unwrap();
        }),
        ("removed", |dir| {
            std::fs::remove_file(writable(dir.join("Unit.st"))).unwrap();
        }),
    ];
    for (change, make) in changes {
        let dir = install(&cache).unwrap();
        make(&dir);
        assert_ne!(db::cache::read(&dir).unwrap(), bundle().entries, "{change}");

        assert_eq!(install(&cache).unwrap(), dir, "{change}");
        assert_eq!(db::cache::read(&dir).unwrap(), bundle().entries, "{change}");
        assert_eq!(cached_names(&cache), [bundle().hash.hex()], "{change}");
    }
}

/// Programs that start at the same moment all get the standard library,
/// and the cache ends with one copy of it.
#[test]
fn starts_at_once_end_with_one_copy() {
    let t = tempfile::tempdir().unwrap();
    let cache = t.path().join("cache");
    let dirs: Vec<_> = std::thread::scope(|scope| {
        let starts: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| install(&cache).unwrap()))
            .collect();
        starts
            .into_iter()
            .map(|start| start.join().unwrap())
            .collect()
    });
    assert!(dirs.iter().all(|dir| *dir == dirs[0]), "{dirs:?}");
    assert_eq!(db::cache::read(&dirs[0]).unwrap(), bundle().entries);
    assert_eq!(cached_names(&cache), [bundle().hash.hex()]);
}

/// A workspace reads the standard library from its copy in the cache.
/// `RK_STDLIB_PATH` names another folder to read in its place, and an
/// empty one reads none.
#[test]
fn a_workspace_reads_the_standard_library_from_the_cache() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let cache = root.join("cache");
    let ws = root.join("ws");
    std::fs::create_dir(&ws).unwrap();
    std::fs::write(
        ws.join("config.toml"),
        "[project]\nname = \"app\"\nversion = \"1\"\n",
    )
    .unwrap();

    built_in(&cache);
    let (db, notices) = open(&ws);
    assert_eq!(notices, [] as [String; 0]);
    let copy = cached_dir(&cache, &bundle().hash);
    let mut files: Vec<String> = db
        .get_library_files()
        .iter()
        .map(|entry| {
            let path = entry.key().to_file_path().unwrap();
            let name = path.strip_prefix(&copy).unwrap();
            name.to_string_lossy().into_owned()
        })
        .collect();
    files.sort();
    let sources: Vec<&String> = bundle()
        .entries
        .keys()
        .filter(|name| name.ends_with(".st"))
        .collect();
    assert_eq!(files.iter().collect::<Vec<_>>(), sources);

    let other = root.join("other");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("Std.st"), "FUNCTION mine : INT\nEND_FUNCTION\n").unwrap();
    // SAFETY: nextest runs each test in a process of its own.
    unsafe { std::env::set_var(db::stdlib::PATH_ENV, &other) };
    let (db, _) = open(&ws);
    let urls: Vec<_> = db
        .get_library_files()
        .iter()
        .map(|entry| entry.key().to_file_path().unwrap())
        .collect();
    assert_eq!(urls, [other.join("Std.st")]);

    unsafe { std::env::set_var(db::stdlib::PATH_ENV, "") };
    let (db, _) = open(&ws);
    assert!(db.get_library_files().is_empty());
}

/// The standard library's own workspace reads no copy of itself: its
/// `.env` sets `RK_STDLIB_PATH` empty, so nothing in it is declared twice.
#[test]
fn the_standard_library_reads_no_copy_of_itself() {
    let t = tempfile::tempdir().unwrap();
    built_in(&t.path().join("cache"));
    assert!(matches!(locate(Some(&stdlib_folder())), Standard::Disabled));

    let (db, notices) = open(&stdlib_folder());
    assert_eq!(notices, [] as [String; 0]);
    assert!(db.get_library_files().is_empty());
    let sources = bundle()
        .entries
        .keys()
        .filter(|name| name.ends_with(".st"))
        .count();
    assert_eq!(db.get_files().len(), sources);
    let errors: Vec<_> = rk::diagnostics::collect_diagnostics(&db, false)
        .into_iter()
        .flat_map(|(_, diagnostics)| diagnostics)
        .filter(|diagnostic| {
            diagnostic.diagnostic.severity == Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        })
        .map(|diagnostic| diagnostic.diagnostic.message)
        .collect();
    assert_eq!(errors, [] as [String; 0]);
}

/// A cache that cannot be written leaves the workspace without its
/// standard library, and says why. Nothing else stops.
#[test]
fn a_cache_that_cannot_be_written_is_reported() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    // A file where the cache would be a folder.
    let cache = root.join("cache");
    std::fs::write(&cache, "").unwrap();
    let ws = root.join("ws");
    std::fs::create_dir(&ws).unwrap();
    std::fs::write(
        ws.join("config.toml"),
        "[project]\nname = \"app\"\nversion = \"1\"\n",
    )
    .unwrap();
    std::fs::write(ws.join("main.st"), "FUNCTION main : INT\nEND_FUNCTION\n").unwrap();

    built_in(&cache);
    let (db, notices) = open(&ws);
    assert!(db.get_library_files().is_empty());
    assert_eq!(db.get_files().len(), 1);
    let [notice] = notices.as_slice() else {
        panic!("one notice: {notices:?}")
    };
    let expected = format!(
        "the standard library could not be installed, so Std.* names will not resolve: \
         unable to write in the cache at '{}': ",
        packages_dir(&cache).display()
    );
    assert!(notice.starts_with(&expected), "{notice}");
}

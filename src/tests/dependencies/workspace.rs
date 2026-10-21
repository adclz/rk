// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A workspace that depends on libraries: the `[dependencies]` of its
//! `config.toml`, read from a folder or fetched into the cache, each
//! library closed to the others as it is to the workspace.

use std::path::{Path, PathBuf};

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use db::dependencies::ContentHash;
use db::{RootDatabase, WorkspaceDataBase};
use insta::assert_snapshot;
use rk::cli::OutputFormat;
use rk::diagnostics::DiagnosticReporter;

/// Writes `files` under `root`, each a path and its text.
fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

fn config(name: &str, dependencies: &str) -> String {
    format!("[project]\nname = \"{name}\"\nversion = \"1.0.0\"\n\n[dependencies]\n{dependencies}")
}

/// No standard library, so that a test does not read the developer's.
fn disable_stdlib() {
    // SAFETY: nextest runs each test in a process of its own.
    unsafe { std::env::set_var(db::stdlib::PATH_ENV, "") };
}

/// A folder for the test, canonical, with a cache of its own in it and no
/// standard library.
fn sandbox() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    disable_stdlib();
    unsafe {
        std::env::set_var(db::cache::CACHE_DIR_ENV, root.join("cache"));
        std::env::remove_var(db::cache::NO_DOWNLOAD_ENV);
    }
    (dir, root)
}

/// The workspace at `root`, opened as `rk` opens it, and what it reports on
/// its `config.toml`.
fn open(root: &Path) -> (RootDatabase, String) {
    // No color: the tests' stderr is not a terminal.
    rk::ui::init_output(OutputFormat::Full);
    let mut db = RootDatabase::default();
    let mut errors = vec![];
    let mut fetcher = rk::workspace::fetcher(db::cache::may_download());
    db::loader::open_workspace(
        &mut db,
        Some(root),
        PositionEncodingKind::UTF8,
        hir::front_end::parser_of,
        fetcher
            .as_mut()
            .map(|fetch| fetch as db::dependencies::Fetch<'_>),
        &mut errors,
        &mut vec![],
    );
    let config_file = root.join("config.toml");
    let mut out = Vec::new();
    DiagnosticReporter::new(&db, root).report_external(
        &Url::from_file_path(&config_file).unwrap(),
        &std::fs::read_to_string(&config_file).unwrap(),
        &errors,
        &mut out,
    );
    (db, String::from_utf8(out).unwrap())
}

/// The errors of the workspace's files and of the libraries', one line
/// each.
fn errors(db: &RootDatabase, root: &Path) -> String {
    let mut per_file = rk::diagnostics::collect_diagnostics(db, false);
    per_file.extend(rk::diagnostics::collect_library_errors(db));
    let mut out = Vec::new();
    DiagnosticReporter::new(db, root)
        .with_format(OutputFormat::Concise)
        .report_files(&per_file, &mut out);
    String::from_utf8(out).unwrap()
}

/// The library each loaded file is of, by its file name: `None` for a
/// file of the workspace.
fn origins(db: &RootDatabase) -> Vec<(String, Option<String>)> {
    let libraries = db::workspace::Workspace::get(db).libraries(db);
    let name = |url: &Url| {
        url.path_segments()
            .unwrap()
            .next_back()
            .unwrap()
            .to_string()
    };
    let mut origins: Vec<_> = db
        .get_files()
        .iter()
        .chain(db.get_library_files().iter())
        .map(|entry| {
            let library = db
                .library_of(*entry.value())
                .map(|id| libraries[id as usize].name.clone());
            (name(entry.key()), library)
        })
        .collect();
    origins.sort();
    origins
}

const MOTION: &str = "NAMESPACE Motion
    FUNCTION Twice : INT
    VAR_INPUT n : INT; END_VAR
        Twice := Helpers.Double(n);
    END_FUNCTION

    NAMESPACE INTERNAL Helpers
        FUNCTION Double : INT
        VAR_INPUT n : INT; END_VAR
            Double := n * 2;
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
";

/// A `path` names a folder beside the workspace, or inside it. Its files
/// are the library's and not the workspace's, and what the library itself
/// depends on is read with it.
#[test]
fn a_workspace_reads_the_libraries_it_names() {
    let (_dir, root) = sandbox();
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config(
                    "ws",
                    "motion = { path = \"../motion\" }\nlocal = { path = \"libs/local\" }\n",
                ),
            ),
            (
                "ws/main.st",
                "USING Motion;\nFUNCTION Run : INT\n    Run := Twice(2) + Thrice(Seven());\nEND_FUNCTION\n",
            ),
            ("ws/libs/local/config.toml", &config("local", "")),
            (
                "ws/libs/local/Local.st",
                "FUNCTION Seven : INT\n    Seven := 7;\nEND_FUNCTION\n",
            ),
            (
                "motion/config.toml",
                &config("motion", "units = { path = \"vendor/units\" }\n"),
            ),
            ("motion/Motion.st", MOTION),
            ("motion/vendor/units/config.toml", &config("units", "")),
            (
                "motion/vendor/units/Units.st",
                "FUNCTION Thrice : INT\nVAR_INPUT n : INT; END_VAR\n    Thrice := n * 3;\nEND_FUNCTION\n",
            ),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    assert_eq!(errors(&db, &ws), "");
    let of = |library: &str| Some(library.to_string());
    assert_eq!(
        origins(&db),
        [
            ("Local.st".to_string(), of("local")),
            ("Motion.st".to_string(), of("motion")),
            ("Units.st".to_string(), of("units")),
            ("main.st".to_string(), None),
        ]
    );
}

/// A file is the workspace's where the loader reads it: under the folder,
/// in no folder with a `config.toml` of its own, and not a library's. The
/// language server asks it of what the editor opens and the watcher
/// reports, a file the library gains once the workspace is open among them.
#[test]
fn a_file_is_the_workspace_s_where_the_loader_reads_it() {
    let (_dir, root) = sandbox();
    let pou = |name: &str| format!("FUNCTION {name} : INT\n    {name} := 1;\nEND_FUNCTION\n");
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "local = { path = \"libs/local\" }\n"),
            ),
            ("ws/main.st", &pou("Run")),
            ("ws/src/More.st", &pou("More")),
            ("ws/libs/local/config.toml", &config("local", "")),
            ("ws/libs/local/Local.st", &pou("Seven")),
            ("ws/other/config.toml", &config("other", "")),
            ("ws/other/Other.st", &pou("Other")),
            ("elsewhere/Far.st", &pou("Far")),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    assert_eq!(
        origins(&db),
        [
            ("Local.st".to_string(), Some("local".to_string())),
            ("More.st".to_string(), None),
            ("main.st".to_string(), None),
        ]
    );
    write(&ws, &[("libs/local/Later.st", &pou("Later"))]);

    let is_workspace_file = |path: &str| db::loader::is_workspace_file(&db, &root.join(path));
    assert!(is_workspace_file("ws/main.st"));
    assert!(is_workspace_file("ws/src/More.st"));
    assert!(!is_workspace_file("ws/libs/local/Local.st"));
    assert!(!is_workspace_file("ws/libs/local/Later.st"));
    assert!(!is_workspace_file("ws/other/Other.st"));
    assert!(!is_workspace_file("elsewhere/Far.st"));
    assert!(!is_workspace_file("ws/missing.st"));
}

/// `INTERNAL` stops at the library: what one library keeps to itself is
/// closed to another library as it is to the workspace, and open to its
/// own files.
#[test]
fn a_library_is_closed_to_another_library() {
    let (_dir, root) = sandbox();
    let uses = |function: &str| {
        format!(
            "FUNCTION {function} : INT\n    {function} := Motion.Helpers.Double(1) + Motion.Twice(1);\nEND_FUNCTION\n"
        )
    };
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config(
                    "ws",
                    "motion = { path = \"../motion\" }\nother = { path = \"../other\" }\n",
                ),
            ),
            ("ws/main.st", &uses("FromTheWorkspace")),
            ("motion/config.toml", &config("motion", "")),
            ("motion/Motion.st", MOTION),
            (
                "other/config.toml",
                &config("other", "motion = { path = \"../motion\" }\n"),
            ),
            ("other/Other.st", &uses("FromAnotherLibrary")),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    let errors = errors(&db, &ws);
    let lines: Vec<&str> = errors.lines().collect();
    assert_eq!(lines.len(), 2, "{errors}");
    assert!(lines.iter().all(|line| line.contains("E1004")), "{errors}");
    assert!(
        lines.iter().any(|line| line.contains("main.st")),
        "{errors}"
    );
    assert!(
        lines.iter().any(|line| line.contains("Other.st")),
        "{errors}"
    );
}

/// Two libraries that declare the same name do not clash: each library's
/// name is the namespace its declarations are in. The bare name is
/// ambiguous, and the library's name before it says which.
#[test]
fn a_name_two_libraries_declare_is_ambiguous_until_qualified() {
    let (_dir, root) = sandbox();
    let clash = "FUNCTION Clash : INT\n    Clash := 1;\nEND_FUNCTION\n";
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "a = { path = \"../a\" }\nb = { path = \"../b\" }\n"),
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := Clash();\nEND_FUNCTION\n\
                 FUNCTION Sure : INT\n    Sure := A.Clash() + B.Clash();\nEND_FUNCTION\n",
            ),
            ("a/config.toml", &config("a", "")),
            ("a/A.st", clash),
            ("b/config.toml", &config("b", "")),
            ("b/B.st", clash),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    let errors = errors(&db, &ws);
    assert_eq!(errors.lines().count(), 1, "{errors}");
    assert!(
        errors.contains("main.st:2") && errors.contains("E0205"),
        "{errors}"
    );
}

/// A namespace two libraries declare is the same: ambiguous until the
/// library's name is written before it, in a `USING` as in a path.
#[test]
fn a_namespace_two_libraries_declare_is_ambiguous_until_qualified() {
    let (_dir, root) = sandbox();
    let motion = |n: i32| {
        format!(
            "NAMESPACE Motion
    FUNCTION Twice : INT
        Twice := {n};
    END_FUNCTION
END_NAMESPACE
"
        )
    };
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "a = { path = \"../a\" }\nb = { path = \"../b\" }\n"),
            ),
            (
                "ws/main.st",
                "USING Motion;\nFUNCTION Run : INT\n    Run := Motion.Twice();\nEND_FUNCTION\n\
                 FUNCTION Sure : INT\n    Sure := A.Motion.Twice() + B.Motion.Twice();\nEND_FUNCTION\n",
            ),
            ("a/config.toml", &config("a", "")),
            ("a/A.st", &motion(1)),
            ("b/config.toml", &config("b", "")),
            ("b/B.st", &motion(2)),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    assert_snapshot!(errors(&db, &ws), @r"
    main.st:1:7: error[E0212]: 'Motion' is 'a.Motion' and 'b.Motion'
    main.st:3:19: error[E0212]: 'Motion' is 'a.Motion' and 'b.Motion'
    ");
}

/// A library is named by a namespace name: `press-line` cannot be one.
#[test]
fn a_library_name_that_is_no_namespace_name_is_refused() {
    let (_dir, root) = sandbox();
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "press-line = { path = \"../press-line\" }\n"),
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := 1;\nEND_FUNCTION\n",
            ),
            ("press-line/config.toml", &config("press-line", "")),
            (
                "press-line/Line.st",
                "FUNCTION Go : INT\n    Go := 1;\nEND_FUNCTION\n",
            ),
        ],
    );
    let ws = root.join("ws");
    let (_db, reported) = open(&ws);
    for expected in [
        "config.toml:6:1",
        "'press-line' is not a namespace name",
        "Help: name it 'press_line'",
        "Note: a library's name is the namespace its declarations are in",
    ] {
        assert!(reported.contains(expected), "{expected}:\n{reported}");
    }
}

/// An entry written wrong is reported on its own line of `config.toml`,
/// and the entries beside it are still read.
#[test]
fn an_entry_written_wrong_is_reported_where_it_is() {
    let (_dir, root) = sandbox();
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config(
                    "ws",
                    "good = { path = \"../good\" }
nothing = {}
both = { path = \"../good\", url = \"https://example.org/x.tar.gz\" }
hashed = { path = \"../good\", hash = \"sha256:00\" }
unhashed = { url = \"https://example.org/x.tar.gz\" }
itself = { path = \".\" }
",
                ),
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := One();\nEND_FUNCTION\n",
            ),
            ("good/config.toml", &config("good", "")),
            (
                "good/Good.st",
                "FUNCTION One : INT\n    One := 1;\nEND_FUNCTION\n",
            ),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(errors(&db, &ws), "");
    assert_snapshot!(reported, @r#"
    Error: 'nothing' has no `path` and no `url`
       ╭─[ config.toml:7:1 ]
       │
     7 │ nothing = {}
       │ ───┬───  
       │    ╰───── 'nothing' has no `path` and no `url`
       │ 
       │ Note: a dependency is a folder (`path`) or an archive (`url` and `hash`)
    ───╯
    Error: 'both' has a `path` and a `url`
       ╭─[ config.toml:8:1 ]
       │
     8 │ both = { path = "../good", url = "https://example.org/x.tar.gz" }
       │ ──┬─  
       │   ╰─── 'both' has a `path` and a `url`
       │ 
       │ Help: keep one of the two
       │ 
       │ Note: a dependency is a folder (`path`) or an archive (`url` and `hash`)
    ───╯
    Error: 'hashed' has a `path` and a `hash`
       ╭─[ config.toml:9:1 ]
       │
     9 │ hashed = { path = "../good", hash = "sha256:00" }
       │ ───┬──  
       │    ╰──── 'hashed' has a `path` and a `hash`
       │ 
       │ Help: remove the `hash`
       │ 
       │ Note: a `hash` goes with a `url`
    ───╯
    Error: 'unhashed' has no `hash`
        ╭─[ config.toml:10:1 ]
        │
     10 │ unhashed = { url = "https://example.org/x.tar.gz" }
        │ ────┬───  
        │     ╰───── 'unhashed' has no `hash`
        │ 
        │ Help: run `rk fetch --save https://example.org/x.tar.gz`
        │ 
        │ Note: a `url` goes with the `hash` of what it serves
    ────╯
    Error: 'itself' is the workspace itself
        ╭─[ config.toml:11:1 ]
        │
     11 │ itself = { path = "." }
        │ ───┬──  
        │    ╰──── 'itself' is the workspace itself
    ────╯
    "#);
}

/// A `.tar.gz` of `files`, under one folder as a host makes it of a tag.
fn archive(root: &str, files: &[(&str, &str)]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for (path, text) in files {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(text.len() as u64);
        tar.append_data(&mut header, format!("{root}/{path}"), text.as_bytes())
            .unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// The `file` URL of `bytes`, written under `dir`.
fn served(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    Url::from_file_path(path).unwrap().to_string()
}

fn drives(version: &str, result: &str) -> Vec<u8> {
    archive(
        &format!("drives-{version}"),
        &[
            (
                "config.toml",
                &format!("[project]\nname = \"drives\"\nversion = \"{version}\"\n"),
            ),
            (
                "Drives.st",
                &format!("FUNCTION Speed : INT\n    Speed := {result};\nEND_FUNCTION\n"),
            ),
        ],
    )
}

/// `rk fetch --save` writes the entry with the hash of what the URL
/// serves, and the workspace then reads the library from the cache.
/// Fetching the next version replaces the entry.
#[test]
fn a_library_is_fetched_and_saved() {
    let (_dir, root) = sandbox();
    let ws = root.join("ws");
    write(
        &root,
        &[
            (
                "ws/config.toml",
                "# The press line.\n[project]\nname = \"ws\"\nversion = \"1.0.0\"\n",
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := Speed();\nEND_FUNCTION\n",
            ),
        ],
    );
    let saved = |url: &str| {
        rk::commands::fetch::run_fetch(&ws, Some(url), OutputFormat::Concise).expect("fetched");
        let text = std::fs::read_to_string(ws.join("config.toml")).unwrap();
        let hash = text
            .split("hash = \"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        (text.clone(), ContentHash::parse(hash).expect("a hash"))
    };

    let first = served(&root, "drives-1.tar.gz", &drives("1.0.0", "10"));
    let (text, first_hash) = saved(&first);
    assert_eq!(
        text,
        format!(
            "# The press line.\n[project]\nname = \"ws\"\nversion = \"1.0.0\"\n\n[dependencies]\ndrives = {{ url = \"{first}\", hash = \"{first_hash}\" }}\n"
        )
    );
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    assert_eq!(errors(&db, &ws), "");
    let library = &db::workspace::Workspace::get(&db).libraries(&db)[0];
    assert_eq!(library.name, "drives");
    assert_eq!(
        library.dir,
        db::dependencies::cached_dir(&root.join("cache"), &first_hash)
    );

    let second = served(&root, "drives-2.tar.gz", &drives("2.0.0", "20"));
    let (text, second_hash) = saved(&second);
    assert_ne!(second_hash, first_hash);
    assert_eq!(text.matches("drives = ").count(), 1, "{text}");
    assert!(text.contains(&second), "{text}");
    assert_eq!(open(&ws).1, "");
}

/// A library the cache lacks is fetched when the workspace is opened, and
/// reported when nothing may be downloaded. `rk fetch` fetches it all the
/// same: it was asked to.
#[test]
fn a_library_the_cache_lacks_is_fetched_or_reported() {
    let (_dir, root) = sandbox();
    let ws = root.join("ws");
    let url = served(&root, "drives.tar.gz", &drives("1.0.0", "10"));
    // The hash, from a cache of its own.
    let hash = fetch::fetch(&root.join("elsewhere"), &url).unwrap().hash;
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config(
                    "ws",
                    &format!("drives = {{ url = \"{url}\", hash = \"{hash}\" }}\n"),
                ),
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := Speed();\nEND_FUNCTION\n",
            ),
        ],
    );

    unsafe { std::env::set_var(db::cache::NO_DOWNLOAD_ENV, "1") };
    let (_, reported) = open(&ws);
    assert!(
        reported.contains("'drives' is not in the cache"),
        "{reported}"
    );
    assert!(reported.contains("run `rk fetch`"), "{reported}");
    assert!(!root.join("cache").exists());

    rk::commands::fetch::run_fetch(&ws, None, OutputFormat::Concise).expect("fetched");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    assert_eq!(errors(&db, &ws), "");

    // And with no one refusing, opening the workspace is enough.
    unsafe {
        std::env::remove_var(db::cache::NO_DOWNLOAD_ENV);
        std::env::set_var(db::cache::CACHE_DIR_ENV, root.join("another"));
    }
    assert_eq!(open(&ws).1, "");
    assert!(db::dependencies::cached_dir(&root.join("another"), &hash).is_dir());
}

/// A `url` moved to a newer archive, with the hash of the older one left
/// beside it, is not the pair the cache has. It is fetched, and what the
/// URL serves is not what the hash says.
#[test]
fn a_hash_left_behind_by_its_url_is_refused() {
    let (_dir, root) = sandbox();
    let ws = root.join("ws");
    write(
        &root,
        &[
            ("ws/config.toml", &config("ws", "")),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := Speed();\nEND_FUNCTION\n",
            ),
        ],
    );
    let old = served(&root, "drives-1.tar.gz", &drives("1.0.0", "10"));
    let new = served(&root, "drives-2.tar.gz", &drives("2.0.0", "20"));
    rk::commands::fetch::run_fetch(&ws, Some(&old), OutputFormat::Concise).expect("fetched");
    assert_eq!(open(&ws).1, "");

    let config_file = ws.join("config.toml");
    let text = std::fs::read_to_string(&config_file).unwrap();
    std::fs::write(&config_file, text.replace(&old, &new)).unwrap();

    let (_, reported) = open(&ws);
    assert!(
        reported.contains("'drives' could not be fetched"),
        "{reported}"
    );
    assert!(
        reported.contains("hash mismatch: config.toml declares sha256:"),
        "{reported}"
    );

    unsafe { std::env::set_var(db::cache::NO_DOWNLOAD_ENV, "1") };
    let (_, reported) = open(&ws);
    assert!(
        reported.contains("'drives' is not in the cache"),
        "{reported}"
    );
}

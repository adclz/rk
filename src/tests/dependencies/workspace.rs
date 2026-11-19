// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A workspace that depends on libraries: the `[dependencies]` of its
//! `config.toml`, read from a folder or fetched into the cache, each
//! library closed to the others as it is to the workspace.

use auto_lsp::lsp_types::{PositionEncodingKind, Url};
use db::dependencies::ContentHash;
use db::{RootDatabase, WorkspaceDataBase};
use insta::assert_snapshot;
use rk::cli::OutputFormat;
use rk::diagnostics::DiagnosticReporter;
use std::path::{Path, PathBuf};

/// Writes `files` under `root`, each a path and its text.
fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// A program's `config.toml`, with the `[dependencies]` given.
fn config(name: &str, dependencies: &str) -> String {
    format!("[project]\nname = \"{name}\"\nversion = \"1.0.0\"\n\n[dependencies]\n{dependencies}")
}

/// A library's `config.toml`, with the `[dependencies]` given.
fn library_config(name: &str, dependencies: &str) -> String {
    format!(
        "[project]\nname = \"{name}\"\nversion = \"1.0.0\"\ntype = \"library\"\n\n[dependencies]\n{dependencies}"
    )
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
    let mut db = RootDatabase::default();
    let reported = open_in(&mut db, root);
    (db, reported)
}

/// The workspace at `root` opened in `db`, which may hold it already, as
/// the language server opens it again when its `config.toml` changes, and
/// what it reports on its `config.toml`.
fn open_in(db: &mut RootDatabase, root: &Path) -> String {
    // No color: the tests' stderr is not a terminal.
    rk::ui::init_output(OutputFormat::Full);
    let mut errors = vec![];
    let mut fetcher = rk::workspace::fetcher(db::cache::may_download());
    db::loader::open_workspace(
        db,
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
    DiagnosticReporter::new(db, root).report_external(
        &Url::from_file_path(&config_file).unwrap(),
        &std::fs::read_to_string(&config_file).unwrap(),
        &errors,
        &mut out,
    );
    String::from_utf8(out).unwrap()
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

/// What `rk check` reports on the workspace's files, the linter's warnings
/// included, one line each.
fn checked(db: &RootDatabase, root: &Path) -> String {
    let per_file = rk::diagnostics::collect_diagnostics(db, true);
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
        .map(|file| {
            let library = db
                .library_of(*file)
                .map(|id| libraries[id as usize].name.clone());
            (name(file.url(db)), library)
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
/// are the library's and not the workspace's.
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
                "USING Motion;\nFUNCTION Run : INT\n    Run := Twice(Seven());\nEND_FUNCTION\n",
            ),
            ("ws/libs/local/config.toml", &library_config("local", "")),
            (
                "ws/libs/local/Local.st",
                "FUNCTION Seven : INT\n    Seven := 7;\nEND_FUNCTION\n",
            ),
            ("motion/config.toml", &library_config("motion", "")),
            ("motion/Motion.st", MOTION),
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
            ("main.st".to_string(), None),
        ]
    );
}

/// A library with dependencies of its own is refused on its line of the
/// workspace's `config.toml`, and neither it nor what it names is read.
#[test]
fn a_library_with_dependencies_is_refused_on_its_line() {
    let (_dir, root) = sandbox();
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "motion = { path = \"../motion\" }\n"),
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := 1;\nEND_FUNCTION\n",
            ),
            (
                "motion/config.toml",
                &library_config("motion", "units = { path = \"vendor/units\" }\n"),
            ),
            ("motion/Motion.st", MOTION),
            (
                "motion/vendor/units/config.toml",
                &library_config("units", ""),
            ),
            (
                "motion/vendor/units/Units.st",
                "FUNCTION Thrice : INT\nVAR_INPUT n : INT; END_VAR\n    Thrice := n * 3;\nEND_FUNCTION\n",
            ),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(origins(&db), [("main.st".to_string(), None)]);
    assert_snapshot!(reported, @r#"
    Error: 'motion' has dependencies of its own
       ╭─[ config.toml:6:1 ]
       │
     6 │ motion = { path = "../motion" }
       │ ───┬──  
       │    ╰──── 'motion' has dependencies of its own
       │ 
       │ Note: a library has no `[dependencies]`: the workspace lists every library it reads
    ───╯
    "#);
}

/// A library checked in its own folder keeps a library's rules: an entry of
/// its `[dependencies]` is refused on its line and nothing is read for it,
/// and an address in its code is E1432, as when a program builds with it,
/// where the linter's warning steps aside. The same folder said to be a
/// program, opened again as the language server opens a changed
/// configuration, has neither rule, and the warning is back.
#[test]
fn a_library_keeps_its_rules_in_its_own_folder() {
    let (_dir, root) = sandbox();
    let axis =
        "FUNCTION_BLOCK Axis\nVAR_OUTPUT q : BOOL; END_VAR\n    q := %IX0.0;\nEND_FUNCTION_BLOCK\n";
    write(
        &root,
        &[
            (
                "drives/config.toml",
                &library_config("drives", "units = { path = \"../units\" }\n"),
            ),
            ("drives/Axis.st", axis),
            ("units/config.toml", &library_config("units", "")),
            (
                "units/Units.st",
                "FUNCTION Thrice : INT\nVAR_INPUT n : INT; END_VAR\n    Thrice := n * 3;\nEND_FUNCTION\n",
            ),
        ],
    );
    let drives = root.join("drives");
    let (db, reported) = open(&drives);
    assert_eq!(origins(&db), [("Axis.st".to_string(), None)]);
    assert_snapshot!(reported, @r#"
    Error: 'units' is a dependency of a library
       ╭─[ config.toml:7:1 ]
       │
     7 │ units = { path = "../units" }
       │ ──┬──  
       │   ╰──── 'units' is a dependency of a library
       │ 
       │ Help: remove this entry
       │ 
       │ Note: a library has no `[dependencies]`: a program lists every library it reads
    ───╯
    "#);
    assert_snapshot!(errors(&db, &drives), @"Axis.st:3:10: error[E1432]: '%IX0.0' is an address in a library");
    // The error says it all: the linter's warning about the same address
    // steps aside.
    assert_snapshot!(checked(&db, &drives), @"Axis.st:3:10: error[E1432]: '%IX0.0' is an address in a library");

    // Opened again, as the language server does when `config.toml` changes.
    write(&root, &[("drives/config.toml", &config("drives", ""))]);
    let mut db = db;
    let reported = open_in(&mut db, &drives);
    assert_eq!(reported, "");
    assert_eq!(errors(&db, &drives), "");
    assert_snapshot!(checked(&db, &drives), @"Axis.st:3:10: warning[L0132]: the address '%IX0.0' is used directly");
}

/// A library declares no CONFIGURATION (E1433), whose globals would be no
/// program's: the program's VAR_EXTERNAL does not find them (E0206), and
/// the library's own block takes no global at all (E1008). The
/// configuration is refused whole: the unknown program its resource runs is
/// not reported beside it. In the library's own folder the errors are the
/// same.
#[test]
fn a_library_declares_no_configuration() {
    let (_dir, root) = sandbox();
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "lib = { path = \"../lib\" }\n"),
            ),
            (
                "ws/main.st",
                "PROGRAM Main\nVAR_EXTERNAL libg : INT; END_VAR\nVAR x : INT; END_VAR\n    x := libg;\nEND_PROGRAM\n",
            ),
            ("lib/config.toml", &library_config("lib", "")),
            (
                "lib/L.st",
                "CONFIGURATION LibCell\nVAR_GLOBAL libg : INT := 5; END_VAR\n\
                 \x20   RESOURCE Res ON PLC\n\
                 \x20       TASK T(INTERVAL := T#10ms, PRIORITY := 1);\n\
                 \x20       PROGRAM Inst WITH T : Nope;\n\
                 \x20   END_RESOURCE\nEND_CONFIGURATION\n\n\
                 FUNCTION_BLOCK UsesLibGlobal\nVAR_EXTERNAL libg : INT; END_VAR\nVAR_OUTPUT q : INT; END_VAR\n    q := libg;\nEND_FUNCTION_BLOCK\n",
            ),
        ],
    );
    // A library's file is shown by its URL, under the test's folder.
    let under_root = format!("{}/", Url::from_file_path(&root).unwrap());
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    assert_snapshot!(errors(&db, &ws).replace(&under_root, ""), @r"
    main.st:2:14: error[E0206]: no VAR_GLOBAL is named 'libg'
    lib/L.st:1:15: error[E1433]: 'LibCell' is a CONFIGURATION in a library
    lib/L.st:10:14: error[E1008]: 'libg' is a global
    ");

    let lib = root.join("lib");
    let (db, reported) = open(&lib);
    assert_eq!(reported, "");
    assert_snapshot!(errors(&db, &lib), @r"
    L.st:1:15: error[E1433]: 'LibCell' is a CONFIGURATION in a library
    L.st:10:14: error[E1008]: 'libg' is a global
    ");
}

/// Two entries whose names differ only in case are one name: each is
/// refused on its own line, and neither library is read.
#[test]
fn two_names_that_differ_only_in_case_are_refused_on_their_lines() {
    let (_dir, root) = sandbox();
    let pou = |name: &str| format!("FUNCTION {name} : INT\n    {name} := 1;\nEND_FUNCTION\n");
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config(
                    "ws",
                    "Drives = { path = \"../a\" }\ndrives = { path = \"../b\" }\n",
                ),
            ),
            ("ws/main.st", &pou("Run")),
            ("a/config.toml", &library_config("Drives", "")),
            ("a/A.st", &pou("F")),
            ("b/config.toml", &library_config("drives", "")),
            ("b/B.st", &pou("G")),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(origins(&db), [("main.st".to_string(), None)]);
    assert_snapshot!(reported, @r#"
    Error: 'Drives' and 'drives' are one name
       ╭─[ config.toml:6:1 ]
       │
     6 │ Drives = { path = "../a" }
       │ ───┬──  
       │    ╰──── 'Drives' and 'drives' are one name
       │ 
       │ Help: keep one of them
       │ 
       │ Note: a library's name is its namespace, and names are case-insensitive
    ───╯
    Error: 'drives' and 'Drives' are one name
       ╭─[ config.toml:7:1 ]
       │
     7 │ drives = { path = "../b" }
       │ ───┬──  
       │    ╰──── 'drives' and 'Drives' are one name
       │ 
       │ Help: keep one of them
       │ 
       │ Note: a library's name is its namespace, and names are case-insensitive
    ───╯
    "#);
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
            ("ws/libs/local/config.toml", &library_config("local", "")),
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

/// `INTERNAL` stops at the library: what a library keeps to itself is
/// closed to the workspace, and open to its own files. Another library sees
/// none of it, public or not: it is another library's (E1008).
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
            ("motion/config.toml", &library_config("motion", "")),
            ("motion/Motion.st", MOTION),
            ("other/config.toml", &library_config("other", "")),
            ("other/Other.st", &uses("FromAnotherLibrary")),
        ],
    );
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    let under_root = format!("{}/", Url::from_file_path(&root).unwrap());
    assert_snapshot!(errors(&db, &ws).replace(&under_root, ""), @r"
    main.st:2:40: error[E1004]: cannot access INTERNAL namespace 'Motion.Helpers'
    other/Other.st:2:42: error[E1008]: 'Double' is declared by the library 'motion'
    other/Other.st:2:61: error[E1008]: 'Twice' is declared by the library 'motion'
    ");
}

/// A library sees its own declarations and the standard library's, and
/// nothing else, whatever the program that reads it lists or declares
/// (E1008): another library's, by its name or bare; the program's
/// namespace, and a block the program opens under the library's own name;
/// a global, by a VAR_EXTERNAL or bare; another library by a USING, an
/// EXTENDS, an IMPLEMENTS or a variable's type. What
/// the program names the same does not stand in front of the standard
/// library's: `Timers.TON` is the standard library's for the library, the
/// program's for the program. An overload the program adds under the
/// library's name is not one the library's call picks from: `Pick` takes
/// the library's INT, and a REAL is refused.
#[test]
fn a_library_sees_only_itself_and_the_standard_library() {
    let (_dir, root) = sandbox();
    let timers = |inputs: &str| {
        format!(
            "NAMESPACE Timers\nFUNCTION_BLOCK TON\nVAR_INPUT {inputs} END_VAR\nEND_FUNCTION_BLOCK\nEND_NAMESPACE\n"
        )
    };
    write(
        &root,
        &[
            // A standard library of the test's own, read in place of the
            // one built in.
            (
                "std/config.toml",
                "[project]\nname = \"Std\"\nversion = \"1\"\ntype = \"library\"\n",
            ),
            ("std/Timers.st", &timers("IN : BOOL; PT : TIME;")),
            (
                "ws/config.toml",
                &config(
                    "ws",
                    "lib = { path = \"../lib\" }\nother = { path = \"../other\" }\n",
                ),
            ),
            (
                "ws/main.st",
                "NAMESPACE Plant\nFUNCTION Secret : INT\n    Secret := 42;\nEND_FUNCTION\nEND_NAMESPACE\n\n\
                 NAMESPACE Lib.Sub\nFUNCTION Slipped : INT\n    Slipped := 1;\nEND_FUNCTION\nEND_NAMESPACE\n\n\
                 FUNCTION_BLOCK Uses\nVAR t : Timers.TON; END_VAR\n    t(IN := TRUE);\nEND_FUNCTION_BLOCK\n\n\
                 CONFIGURATION Cell\nVAR_GLOBAL progg : INT; END_VAR\nEND_CONFIGURATION\n\n\
                 NAMESPACE Lib\nFUNCTION Pick : INT\nVAR_INPUT x : REAL; END_VAR\n    Pick := 2;\nEND_FUNCTION\nEND_NAMESPACE\n",
            ),
            ("ws/timers.st", &timers("IN : BOOL;")),
            ("other/config.toml", &library_config("other", "")),
            (
                "other/O.st",
                "FUNCTION Helper : INT\n    Helper := 7;\nEND_FUNCTION\n\n\
                 FUNCTION_BLOCK Base\nEND_FUNCTION_BLOCK\n\nINTERFACE IRun\nEND_INTERFACE\n\n\
                 TYPE Level : INT; END_TYPE\n",
            ),
            ("lib/config.toml", &library_config("lib", "")),
            (
                "lib/L.st",
                "NAMESPACE Sub\nFUNCTION Own : INT\n    Own := Slipped();\nEND_FUNCTION\nEND_NAMESPACE\n\n\
                 FUNCTION Reaches : INT\n    Reaches := Helper() + Other.Helper() + Plant.Secret();\nEND_FUNCTION\n\n\
                 FUNCTION_BLOCK Globals\nVAR_EXTERNAL progg : DINT; END_VAR\nVAR_OUTPUT q : DINT; END_VAR\n    q := progg;\nEND_FUNCTION_BLOCK\n\n\
                 FUNCTION Bare : INT\n    Bare := progg;\nEND_FUNCTION\n\n\
                 FUNCTION_BLOCK Blink\nVAR t : Timers.TON; END_VAR\n    t(IN := TRUE, PT := T#1s);\nEND_FUNCTION_BLOCK\n\n\
                 FUNCTION Pick : INT\nVAR_INPUT x : INT; END_VAR\n    Pick := 1;\nEND_FUNCTION\n\n\
                 FUNCTION Picks : INT\n    Picks := Pick(x := REAL#1.5);\nEND_FUNCTION\n\n\
                 NAMESPACE Imports\nUSING Other;\nEND_NAMESPACE\n\n\
                 FUNCTION_BLOCK Derived EXTENDS Other.Base IMPLEMENTS Other.IRun\nVAR x : Other.Level; END_VAR\nEND_FUNCTION_BLOCK\n",
            ),
        ],
    );
    // SAFETY: nextest runs each test in a process of its own.
    unsafe { std::env::set_var(db::stdlib::PATH_ENV, root.join("std")) };
    let ws = root.join("ws");
    let (db, reported) = open(&ws);
    assert_eq!(reported, "");
    let under_root = format!("{}/", Url::from_file_path(&root).unwrap());
    assert_snapshot!(errors(&db, &ws).replace(&under_root, ""), @r"
    lib/L.st:3:12: error[E1008]: 'Slipped' is declared by the program
    lib/L.st:8:16: error[E1008]: 'Helper' is declared by the library 'other'
    lib/L.st:8:33: error[E1008]: 'Helper' is declared by the library 'other'
    lib/L.st:8:50: error[E1008]: 'Secret' is declared by the program
    lib/L.st:12:14: error[E1008]: 'progg' is a global
    lib/L.st:18:13: error[E1008]: 'progg' is a global
    lib/L.st:32:24: error[E0301]: expected 'INT', got 'REAL': help: insert explicit cast 'REAL_TO_INT(REAL#1.5)'
    lib/L.st:36:7: error[E1008]: 'Other' is declared by the library 'other'
    lib/L.st:39:32: error[E1008]: 'Other.Base' is declared by the library 'other'
    lib/L.st:39:54: error[E1008]: 'Other.IRun' is declared by the library 'other'
    lib/L.st:40:9: error[E1008]: 'Other.Level' is declared by the library 'other'
    ");
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
            ("a/config.toml", &library_config("a", "")),
            ("a/A.st", clash),
            ("b/config.toml", &library_config("b", "")),
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
            ("a/config.toml", &library_config("a", "")),
            ("a/A.st", &motion(1)),
            ("b/config.toml", &library_config("b", "")),
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
            ("press-line/config.toml", &library_config("press-line", "")),
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
            ("good/config.toml", &library_config("good", "")),
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
        │ Help: run `rk fetch https://example.org/x.tar.gz`
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
                &format!(
                    "[project]\nname = \"drives\"\nversion = \"{version}\"\ntype = \"library\"\n"
                ),
            ),
            (
                "Drives.st",
                &format!("FUNCTION Speed : INT\n    Speed := {result};\nEND_FUNCTION\n"),
            ),
        ],
    )
}

/// `rk fetch <url>` adds the entry with the hash of what the URL serves,
/// and the workspace then reads the library from the cache. Fetching the
/// next version replaces the entry; fetching the same one again writes
/// nothing.
#[test]
fn a_library_is_fetched_and_added() {
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
    // Up to date: the line is left as its author spaced it.
    let respaced = text.replace("drives = { url = ", "drives = {url = ");
    std::fs::write(ws.join("config.toml"), &respaced).unwrap();
    assert_eq!(saved(&first), (respaced, first_hash.clone()), "up to date");

    let second = served(&root, "drives-2.tar.gz", &drives("2.0.0", "20"));
    let (text, second_hash) = saved(&second);
    assert_ne!(second_hash, first_hash);
    assert_eq!(text.matches("drives = ").count(), 1, "{text}");
    assert!(text.contains(&second), "{text}");
    assert_eq!(open(&ws).1, "");
}

/// `rk fetch <url>` checks what it fetched before it writes anything: the
/// entry as the build reads it, and the library's code as `rk check` holds
/// it in its own folder. A refused library leaves `config.toml` as it was.
#[test]
fn a_library_is_checked_before_it_is_added() {
    let (_dir, root) = sandbox();
    let ws = root.join("ws");
    let config = "[project]\nname = \"ws\"\nversion = \"1.0.0\"\n\n[dependencies]\nDRIVES = { path = \"../upper\" }\n";
    let pou = |name: &str| format!("FUNCTION {name} : INT\n    {name} := 1;\nEND_FUNCTION\n");
    write(
        &root,
        &[
            ("ws/config.toml", config),
            ("ws/main.st", &pou("Run")),
            ("upper/config.toml", &library_config("DRIVES", "")),
            ("upper/U.st", &pou("U")),
        ],
    );
    let refused = |archive_name: &str, files: &[(&str, &str)]| {
        let url = served(&root, archive_name, &archive("pkg", files));
        let added = rk::commands::fetch::run_fetch(&ws, Some(&url), OutputFormat::Concise);
        assert!(added.is_err(), "{archive_name} was added");
        assert_eq!(
            std::fs::read_to_string(ws.join("config.toml")).unwrap(),
            config,
            "{archive_name}"
        );
    };

    // An address in its code (E1432).
    refused(
        "address.tar.gz",
        &[
            ("config.toml", &library_config("motion", "")),
            (
                "M.st",
                "FUNCTION_BLOCK M\nVAR_OUTPUT q : BOOL; END_VAR\n    q := %IX0.0;\nEND_FUNCTION_BLOCK\n",
            ),
        ],
    );
    // A folder that does not say it is a library.
    refused(
        "program.tar.gz",
        &[("config.toml", &config_of("plant")), ("P.st", &pou("P"))],
    );
    // A library with dependencies of its own.
    refused(
        "dependencies.tar.gz",
        &[
            (
                "config.toml",
                &library_config("units", "x = { path = \"x\" }\n"),
            ),
            ("U.st", &pou("Thrice")),
        ],
    );
    // A name that is another entry's in another case.
    refused(
        "case.tar.gz",
        &[
            ("config.toml", &library_config("drives", "")),
            ("D.st", &pou("D")),
        ],
    );
}

/// What is checked is the entry being added: another entry, broken before,
/// does not stop it, and stays as it was.
#[test]
fn an_unrelated_broken_entry_does_not_stop_an_addition() {
    let (_dir, root) = sandbox();
    let ws = root.join("ws");
    write(
        &root,
        &[
            (
                "ws/config.toml",
                &config("ws", "stale = { path = \"../nowhere\" }\n"),
            ),
            (
                "ws/main.st",
                "FUNCTION Run : INT\n    Run := 1;\nEND_FUNCTION\n",
            ),
        ],
    );
    let url = served(&root, "drives-1.tar.gz", &drives("1.0.0", "10"));
    rk::commands::fetch::run_fetch(&ws, Some(&url), OutputFormat::Concise).expect("added");
    let text = std::fs::read_to_string(ws.join("config.toml")).unwrap();
    assert!(
        text.contains("stale = { path = \"../nowhere\" }\n"),
        "{text}"
    );
    assert!(
        text.contains(&format!("drives = {{ url = \"{url}\"")),
        "{text}"
    );
}

/// A program's `config.toml` with no dependencies.
fn config_of(name: &str) -> String {
    config(name, "")
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

// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A second front-end, added the way a tool built on rk adds one, and rk's
//! own commands run on a workspace of the two kinds.
//!
//! A test binary of its own, where the other tests live in `src/tests/`:
//! the table of front-ends is the process's and is fixed at its first
//! read, so a test that adds one cannot come, in the same process, after a
//! test that read a file. Each test here starts with [`second`].

// `configure_parser!` (auto-lsp) expands to functions returning
// `Result<_, ParseError>` whose Err variant is ~144 bytes.
#![allow(clippy::result_large_err)]

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};

use ast::generated::SourceFile;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::tracked::ParsedAst;
use auto_lsp::lsp_types::{NumberOrString, PositionEncodingKind, Url};
use db::{RootDatabase, WorkspaceDataBase};
use hir::builder::semantic_index::SemanticIndexBuilder;
use hir::check::errors::ToIdeDiagnostic;
use hir::check::errors::e00_syntax::SyntaxError;
use hir::front_end::FrontEnd;
use hir::hir_def::scope::ScopeId;
use hir::hir_def::semantic_index::{SemanticIndex, semantic_index};
use rk::cli::{Command, OutputFormat};

// rk's own grammar under another parser: a second kind of file needs no
// second language to be one. The parser is what tells the two apart.
auto_lsp::configure_parser!(
    SECOND,
    language: tree_sitter_rk::LANGUAGE,
    ast_root: SourceFile,
);

// A parser no front-end is ever added for.
auto_lsp::configure_parser!(
    NOBODY_S,
    language: tree_sitter_rk::LANGUAGE,
    ast_root: SourceFile,
);

/// How many files the second front-end was asked to index.
static READ: AtomicUsize = AtomicUsize::new(0);

/// The second front-end's index of a file, made the way a front-end
/// outside `hir` makes one: rk's builder, a walk of the file's root that is
/// its own, rk's handler for the node it shares with rk, its own report of
/// what it does not read, and `finish`. It reads a FUNCTION and nothing
/// else.
fn read<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    ast: &'db ParsedAst,
) -> SemanticIndex<'db> {
    type Declaration = ast::generated::ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective;

    READ.fetch_add(1, Ordering::Relaxed);
    let source = ast
        .get_root()
        .and_then(|root| root.downcast_ref::<SourceFile>())
        .expect("an index function is given a tree with its root");
    let mut sema = SemanticIndexBuilder::new(db, file, ast);
    for declaration in source.children.iter() {
        sema.current_scope = ScopeId::global(db, file);
        match declaration.cast(sema.ast) {
            Declaration::FuncDecl(function) => match sema.parse_function(function) {
                Ok(function) => sema.global_pous.push(function),
                Err(fault) => sema.errors.push(fault),
            },
            other => {
                let fault = SyntaxError::SyntaxError {
                    span: *other.get_range(),
                    err: "the second front-end reads a FUNCTION only".to_string(),
                }
                .to_diagnostic(db, file);
                sema.errors.push(fault);
            }
        }
    }
    sema.finish(vec![])
}

/// Adds the second front-end, which reads `.two` files. The first thing a
/// test does.
fn second() {
    // The library is the one a workspace's `.env` names, never the
    // developer's.
    static ENV: Once = Once::new();
    ENV.call_once(|| unsafe { std::env::remove_var(db::loader::STDLIB_PATH_ENV) });
    hir::front_end::register(FrontEnd {
        extensions: &["two"],
        parser: &SECOND,
        index: read,
    });
}

/// A workspace on disk with no library: a `config.toml`, a `.env` saying
/// so, and `files`.
fn workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("workspace tempdir");
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("config.toml"),
        "[project]\nname = \"T\"\nversion = \"0.0\"\n",
    )
    .unwrap();
    std::fs::write(root.join(".env"), "RK_STDLIB_PATH=\n").unwrap();
    for (name, source) in files {
        std::fs::write(root.join(name), source).unwrap();
    }
    (dir, root)
}

fn open(root: &Path) -> (RootDatabase, Vec<File>) {
    let mut db = RootDatabase::default();
    let files = db::loader::open_workspace(
        &mut db,
        Some(root),
        PositionEncodingKind::UTF8,
        &mut vec![],
        &mut vec![],
    )
    .into_iter()
    .map(|file| file.expect("the file is read"))
    .collect();
    (db, files)
}

fn file_named(db: &RootDatabase, root: &Path, name: &str) -> File {
    let url = Url::from_file_path(root.join(name)).unwrap();
    db.get_file(&url)
        .unwrap_or_else(|| panic!("{name} is not in the workspace"))
}

/// The codes of a file's diagnostics.
fn codes(db: &RootDatabase, file: File) -> Vec<String> {
    hir::check::diagnostics_for_file(db, file)
        .iter()
        .filter_map(|diagnostic| match diagnostic.diagnostic.code.as_ref()? {
            NumberOrString::String(code) => Some(code.clone()),
            NumberOrString::Number(code) => Some(code.to_string()),
        })
        .collect()
}

const TWICE: &str = "
FUNCTION Twice : INT
VAR_INPUT x : INT; END_VAR
    Twice := x * 2;
END_FUNCTION
";

/// rk's file calls a FUNCTION the second kind's file declares, and tests
/// what it answers.
const USES_TWICE: &str = "
{test}
FUNCTION twice_is_read
    IF Twice(21) <> 42 THEN
        __RAISE('Twice(21) is not 42');
    END_IF;
END_FUNCTION
";

/// The workspace's loader finds a file by an extension a front-end named,
/// reads it with that front-end's parser, and its index is the one the
/// front-end builds. A name then resolves from one kind to the other.
#[test]
fn a_file_of_the_second_kind_is_found_and_read_by_its_front_end() {
    second();
    let (_dir, root) = workspace(&[
        ("main.st", USES_TWICE),
        ("lib.two", TWICE),
        ("notes.txt", "FUNCTION NotASource END_FUNCTION"),
    ]);
    let (db, files) = open(&root);
    assert_eq!(files.len(), 2, "main.st and lib.two, not notes.txt");

    let main = file_named(&db, &root, "main.st");
    let lib = file_named(&db, &root, "lib.two");
    assert!(db::source::is_rk(main.parsers(&db)));
    assert!(std::ptr::eq(lib.parsers(&db), &*SECOND));

    let before = READ.load(Ordering::Relaxed);
    assert_eq!(semantic_index(&db, lib).global_pous.len(), 1);
    assert!(
        READ.load(Ordering::Relaxed) > before,
        "the second front-end built the index"
    );
    assert_eq!(codes(&db, main), Vec::<String>::new());
    assert_eq!(codes(&db, lib), Vec::<String>::new());
}

/// What a tool built on rk does after it added its front-end: rk's own
/// commands, on a workspace of the two kinds. The test in the `.st` file
/// passes on what the `.two` file computes, so that file was compiled into
/// the module and run.
#[test]
fn rk_s_commands_run_a_workspace_of_the_two_kinds() {
    second();
    let (_dir, root) = workspace(&[("main.st", USES_TWICE), ("lib.two", TWICE)]);
    let run = |command| rk::run(&root, false, OutputFormat::Concise, command);

    assert!(run(Command::Check { watch: false }).is_ok());
    assert!(
        run(Command::Test {
            test_name: None,
            opt_level: None,
            timeout: None,
        })
        .is_ok()
    );
    let module = root.join("out.wasm");
    assert!(
        run(Command::Compile {
            output: Some(module.clone()),
            release: false,
            opt_level: None,
            debug: false,
            watch: false,
        })
        .is_ok()
    );
    assert!(std::fs::read(&module).is_ok_and(|bytes| bytes.starts_with(b"\0asm")));
}

/// The control of the test above: when the `.two` file computes something
/// else, the `.st` file's test fails. Nothing else tells the two runs
/// apart.
#[test]
fn a_test_fails_on_what_the_second_kind_s_file_computes() {
    second();
    let thrice = TWICE.replace("x * 2", "x * 3");
    let (_dir, root) = workspace(&[("main.st", USES_TWICE), ("lib.two", &thrice)]);
    let tested = rk::run(
        &root,
        false,
        OutputFormat::Concise,
        Command::Test {
            test_name: None,
            opt_level: None,
            timeout: None,
        },
    );
    assert!(tested.is_err());
}

/// A fault in the second kind's file is reported like one in rk's: from
/// the index on, the checks are the same.
#[test]
fn a_file_of_the_second_kind_is_checked_like_rk_s() {
    second();
    let wrong = TWICE.replace("x * 2", "'two'");
    let (_dir, root) = workspace(&[("lib.two", &wrong)]);
    let (db, _) = open(&root);
    assert_eq!(codes(&db, file_named(&db, &root, "lib.two")), ["E0308"]);
    assert!(
        rk::run(
            &root,
            false,
            OutputFormat::Concise,
            Command::Check { watch: false }
        )
        .is_err()
    );
}

/// What a front-end does not read is its own to report, where it stands:
/// the fault it put in its index is among the file's diagnostics, and the
/// FUNCTION beside it is read.
#[test]
fn what_the_second_front_end_does_not_read_is_its_own_to_report() {
    second();
    let source = format!("{TWICE}\nPROGRAM NotRead\nEND_PROGRAM\n");
    let (_dir, root) = workspace(&[("lib.two", &source)]);
    let (db, _) = open(&root);
    let lib = file_named(&db, &root, "lib.two");
    assert_eq!(semantic_index(&db, lib).global_pous.len(), 1);
    assert!(semantic_index(&db, lib).programs.is_empty());
    let faults: Vec<String> = hir::check::diagnostics_for_file(&db, lib)
        .iter()
        .map(|fault| fault.diagnostic.message.clone())
        .collect();
    assert_eq!(faults, ["the second front-end reads a FUNCTION only"]);
    assert_eq!(codes(&db, lib), ["E0001"]);
}

/// A tool's `main` is rk's after its front-end was added: the arguments
/// `rk` takes, on a workspace of the two kinds. The second run is the
/// control, on a workspace whose `.two` file holds a fault.
#[test]
fn a_tool_s_main_is_rk_s_after_its_front_end_was_added() {
    second();
    let (_dir, root) = workspace(&[("main.st", USES_TWICE), ("lib.two", TWICE)]);
    let check = |root: &Path| {
        rk::run_args([
            "tool".as_ref(),
            "check".as_ref(),
            "--workspace".as_ref(),
            root.as_os_str(),
        ])
    };
    assert!(check(&root).is_ok());

    let wrong = TWICE.replace("x * 2", "'two'");
    let (_dir, faulty) = workspace(&[("lib.two", &wrong)]);
    assert!(check(&faulty).is_err());
}

/// A pragma rk has no rule for is rk's fault to report in rk's files
/// (E1510). In a file of another kind such a token may be anything of its
/// language, and its front-end says what it makes of it.
#[test]
fn a_pragma_rk_has_no_rule_for_is_reported_in_rk_s_files_only() {
    second();
    let source = "
{attribute 'hide'}
FUNCTION Hidden : INT
    Hidden := 1;
END_FUNCTION
";
    let (_dir, root) = workspace(&[
        ("main.st", source),
        ("lib.two", &source.replace("Hidden", "HiddenToo")),
    ]);
    let (db, _) = open(&root);
    assert_eq!(codes(&db, file_named(&db, &root, "main.st")), ["E1510"]);
    assert_eq!(
        codes(&db, file_named(&db, &root, "lib.two")),
        Vec::<String>::new()
    );
}

/// The formatter reads rk's own tree: `rk fmt` takes the `.st` files and
/// leaves a file of another kind as it is.
#[test]
fn rk_fmt_formats_rk_s_files_only() {
    second();
    let one_line = "FUNCTION g : INT  g := 2; END_FUNCTION\n";
    let other = one_line.replace(" g ", " h ").replace("g :=", "h :=");
    let (_dir, root) = workspace(&[("main.st", one_line), ("lib.two", &other)]);
    assert!(
        rk::run(
            &root,
            false,
            OutputFormat::Concise,
            Command::Fmt { check: false }
        )
        .is_ok()
    );
    assert_ne!(
        std::fs::read_to_string(root.join("main.st")).unwrap(),
        one_line,
        "the one-line function is reflowed"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("lib.two")).unwrap(),
        other
    );
}

/// A library holds files of the second kind as a workspace does.
#[test]
fn a_library_holds_files_of_the_second_kind() {
    second();
    let library = tempfile::tempdir().unwrap();
    let library_root = std::fs::canonicalize(library.path()).unwrap();
    std::fs::write(library_root.join("Lib.two"), TWICE).unwrap();
    let (_dir, root) = workspace(&[("main.st", USES_TWICE)]);
    std::fs::write(
        root.join(".env"),
        format!("RK_STDLIB_PATH=\"{}\"\n", library_root.display()),
    )
    .unwrap();

    let (db, files) = open(&root);
    assert_eq!(files.len(), 1, "the workspace's own file");
    let lib = Url::from_file_path(library_root.join("Lib.two")).unwrap();
    assert!(db.get_library_files().contains_key(&lib));
    assert_eq!(
        codes(&db, file_named(&db, &root, "main.st")),
        Vec::<String>::new()
    );
}

/// A file keeps the parser that read it, and a parser no front-end was
/// added for reads a file nothing indexes: it is empty, where rk's parser
/// on the same text gives a FUNCTION.
#[test]
fn a_file_whose_parser_has_no_front_end_has_an_empty_index() {
    second();
    let db = RootDatabase::default();
    let file = |name: &str, parser| {
        File::from_string()
            .db(&db)
            .parsers(parser)
            .url(&Url::parse(&format!("file:///{name}")).unwrap())
            .source(TWICE.to_string())
            .call()
            .unwrap()
    };
    let nobody_s = file("a.nobody", &*NOBODY_S);
    let rk_s = file("a.st", &*ast::RK_PARSER);
    assert!(semantic_index(&db, nobody_s).global_pous.is_empty());
    assert_eq!(semantic_index(&db, rk_s).global_pous.len(), 1);
}

/// Files were read with the front-ends there were. One added after would
/// leave them as they are, so it is refused.
#[test]
#[should_panic(expected = "are fixed once a workspace was opened or a file read")]
fn a_front_end_added_after_files_were_read_is_refused() {
    second();
    db::source::kinds();
    hir::front_end::register(FrontEnd {
        extensions: &["late"],
        parser: &NOBODY_S,
        index: read,
    });
}

/// A tool with several entries adds its front-end on each: the same one
/// again is no fault, files read or not.
#[test]
fn the_same_front_end_added_again_changes_nothing() {
    second();
    second();
    assert_eq!(db::source::kinds().len(), 2);
    second();
    assert_eq!(db::source::kinds().len(), 2);
}

#[test]
#[should_panic(expected = "another kind of source file is named '.ST'")]
fn an_extension_names_one_front_end() {
    second();
    hir::front_end::register(FrontEnd {
        extensions: &["ST"],
        parser: &NOBODY_S,
        index: read,
    });
}

/// What is a source file, to the workspace and to what only takes rk's,
/// and how a message names the extensions.
#[test]
fn a_source_file_is_one_a_front_end_named() {
    second();
    assert!(db::loader::is_source_file(Path::new("a.two")));
    assert!(db::loader::is_source_file(Path::new("A.TWO")));
    assert!(db::loader::is_source_file(Path::new("a.st")));
    assert!(!db::loader::is_source_file(Path::new("a.txt")));
    assert!(!db::loader::is_st_file(Path::new("a.two")));
    assert_eq!(db::source::extensions(), ".st or .two");
}

/// `--watch` runs again when a file of the second kind changes, as it
/// does for one of rk's, and still not for a file that is no source.
#[test]
fn a_change_to_a_file_of_the_second_kind_is_watched() {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    second();
    let (_dir, root) = workspace(&[]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let events = seen.clone();
    let watcher = rk::watcher::Watcher::new(&root, move |batch| {
        events.lock().unwrap().extend(batch);
    });
    // The OS watcher takes a moment to be in place, and the debouncer one
    // to deliver, as in the watcher's own tests.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(root.join("lib.two"), TWICE).unwrap();
    std::fs::write(root.join("notes.txt"), "notes").unwrap();
    watcher.flush();
    std::thread::sleep(Duration::from_millis(200));

    let paths: Vec<PathBuf> = seen
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| event.path().map(Path::to_path_buf))
        .collect();
    assert!(
        paths.iter().any(|path| path.ends_with("lib.two")),
        "lib.two is watched: {paths:?}"
    );
    assert!(
        !paths.iter().any(|path| path.ends_with("notes.txt")),
        "notes.txt is not: {paths:?}"
    );
}

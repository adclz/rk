// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! One source in a fresh database, as `rk check` sees a single file, read
//! beside one library.

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, PositionEncodingKind, Url};
use db::{RootDatabase, WorkspaceDataBase};
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;
use ide_diagnostic::IdeDiagnostic;

/// Every input is the same file: a finding reproduces from its text alone.
pub(crate) const URL: &str = "file:///fuzz.st";

pub(crate) fn url() -> Url {
    Url::parse(URL).expect("a valid url")
}

/// The library every input is read with, named `Lib`: a FUNCTION at its
/// top level, two namespaces, and what a workspace cannot reach. A name
/// resolves through a library's root on every input, and a generated
/// program calls these ([`crate::generate`]); a mutated one reaches for
/// `Lib.Flags.Hidden` and `Lib.Keep` and is refused.
pub(crate) const LIBRARY: &str = "FUNCTION Half : DINT
VAR_INPUT n : DINT; END_VAR
    Half := n / 2;
END_FUNCTION

NAMESPACE Math
    FUNCTION Clamp0 : DINT
    VAR_INPUT n : DINT; END_VAR
        IF n < 0 THEN
            Clamp0 := 0;
        ELSE
            Clamp0 := n;
        END_IF;
    END_FUNCTION
END_NAMESPACE

NAMESPACE Flags
    FUNCTION Both : BOOL
    VAR_INPUT a : BOOL; b : BOOL; END_VAR
        Both := a AND b;
    END_FUNCTION

    NAMESPACE INTERNAL Hidden
        FUNCTION Secret : BOOL
            Secret := TRUE;
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE

FUNCTION PRIVATE Keep : DINT
    Keep := 1;
END_FUNCTION
";

pub(crate) const LIBRARY_URL: &str = "file:///lib/Lib.st";

/// A database holding `source` as its only workspace file, beside
/// [`LIBRARY`], or `None` when the parser gives up on it (tree-sitter only
/// does on a timeout or cancellation).
pub(crate) fn load(source: &str) -> Option<(RootDatabase, File)> {
    let mut db = RootDatabase::default();
    // The library's name is the root its declarations are under; the
    // config file named is the standard library's, with no settings, so
    // that no report carries the "no config.toml" hint.
    db::workspace::Workspace::new(
        &db,
        None,
        vec![db::dependencies::Library {
            name: "Lib".to_string(),
            dir: std::path::PathBuf::from("/lib"),
            standard: false,
        }],
        Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/config.toml")),
        PositionEncodingKind::UTF8,
    );
    let library_url = Url::parse(LIBRARY_URL).expect("a valid url");
    let library = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&library_url)
        .source(LIBRARY.to_string())
        .call()
        .ok()?;
    db.insert_library_file(library);

    let url = url();
    let file = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .ok()?;
    db.add_file(file).ok()?;
    let file = db.get_file(&url)?;
    Some((db, file))
}

/// The module of `file` and the library, as `rk compile` lowers a workspace
/// with its libraries.
pub(crate) fn lower(
    db: &RootDatabase,
    file: File,
) -> Result<mir::MirModule, mir::lower::lower_type::LowerTypeError> {
    let mut indices: Vec<_> = db
        .get_library_files()
        .iter()
        .map(|file| semantic_index(db, *file))
        .collect();
    indices.push(semantic_index(db, file));
    mir::lower::lower_module::lower_modules(db, &indices)
}

/// As `rk check` counts: an unset severity is an error by convention.
pub(crate) fn is_error(diagnostic: &IdeDiagnostic) -> bool {
    matches!(
        diagnostic.diagnostic.severity,
        Some(DiagnosticSeverity::ERROR) | None
    )
}

/// Whether `rk compile` would go on to lower this file.
pub(crate) fn compiles(db: &RootDatabase, file: File) -> bool {
    !diagnostics_for_file(db, file).iter().any(is_error)
}

/// The diagnostics as the editor shows them, positions included, in a
/// stable order.
pub(crate) fn positioned_diagnostics(db: &RootDatabase, file: File) -> Vec<String> {
    let mut out: Vec<String> = diagnostics_for_file(db, file)
        .iter()
        .map(|d| format!("{:?}", d.to_lsp_diagnostic(db)))
        .collect();
    out.sort();
    out
}

/// What the diagnostics SAY, without where: formatting moves every span, and
/// only the set of problems must survive it.
pub(crate) fn diagnostic_identities(db: &RootDatabase, file: File) -> Vec<String> {
    let mut out: Vec<String> = diagnostics_for_file(db, file)
        .iter()
        .map(|d| {
            let inner = &d.diagnostic;
            format!("{:?} {:?} {}", inner.code, inner.severity, inner.message)
        })
        .collect();
    out.sort();
    out
}

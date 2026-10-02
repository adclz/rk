// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! One source in a fresh database, as `rk check` sees a single file.

use auto_lsp::default::db::{BaseDatabase, FileManager, file::File};
use auto_lsp::lsp_types::{DiagnosticSeverity, Url};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use ide_diagnostic::IdeDiagnostic;

/// Every input is the same file: a finding reproduces from its text alone.
pub(crate) const URL: &str = "file:///fuzz.st";

pub(crate) fn url() -> Url {
    Url::parse(URL).expect("a valid url")
}

/// A database holding `source` as its only file, or `None` when the parser
/// gives up on it (tree-sitter only does on a timeout or cancellation).
pub(crate) fn load(source: &str) -> Option<(RootDatabase, File)> {
    let mut db = RootDatabase::default();
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
        .map(|d| format!("{:?}", d.to_lsp_diagnostic(db as &dyn BaseDatabase)))
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

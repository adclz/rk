//! The compiler, end to end, on one file: what `rk check`, `rk compile` and
//! a host do with it.

use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;
use wasm_codegen::Profile;

use crate::{Finding, session, wasm};

/// How far a source got. Only a [`Finding`] is a failure; the rest is what
/// the input deserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The parser gave up on it.
    Unparsed,
    /// `rk check` reports an error, so nothing is lowered.
    Rejected,
    /// Lowered, emitted, validated and run.
    Compiled,
}

/// Check, lint, and when the source is accepted, lower, emit both profiles,
/// validate them, decode their sections and run the debug module.
pub fn check(source: &str) -> Result<Verdict, Finding> {
    let Some((db, file)) = session::load(source) else {
        return Ok(Verdict::Unparsed);
    };
    let index = semantic_index(&db, file);
    let _ = diagnostics_for_file(&db, file);

    // Every rule, on every input: the editor lints code that does not
    // compile, and a lint that panics takes the request down with it.
    let mut lints = Vec::new();
    linter::lint_file(&db, file, &every_lint(), &mut lints);

    if !session::compiles(&db, file) {
        return Ok(Verdict::Rejected);
    }

    // `rk check` accepted it, so it must lower: a lowering error on a clean
    // source is what `rk compile` reports as an internal compiler error.
    let mir = mir::lower::lower_module::lower_module(&db, index).map_err(|e| lowering(&e))?;

    let debug = wasm_codegen::generate_wasm_profile(&db, &mir, Profile::Debug).finish();
    let release = wasm_codegen::generate_wasm_profile(&db, &mir, Profile::Release).finish();
    wasm::validate(&debug, "debug")?;
    wasm::validate(&release, "release")?;
    wasm::same_program(&debug, &release)?;
    wasm::decode_sections(&release)?;
    let sections = wasm::decode_sections(&debug)?;
    wasm::execute(&debug, &sections)?;
    Ok(Verdict::Compiled)
}

/// A lowering error on an accepted source, with the line it names: the
/// error text alone does not say which construct it was.
pub(crate) fn lowering(e: &mir::lower::lower_type::LowerTypeError) -> Finding {
    let at = e
        .location()
        .map(|(_, span)| format!(" at line {}", span.start_point.row + 1))
        .unwrap_or_default();
    Finding::new(
        "lowering",
        format!("an accepted source failed to lower{at}: {e}"),
    )
}

fn every_lint() -> db::config_file::LinterConfig {
    let rules = linter::rules::ALL_RULE_NAMES
        .iter()
        .map(|name| (name.to_string(), true))
        .collect();
    db::config_file::LinterConfig {
        select: Some(db::config_file::Select::All),
        rules: Some(rules),
    }
}

//! Does the program compute what it should?
//!
//! A source whose first line is `(* rk-fuzz expects, after N scans:`
//! names, one per line, the value each variable holds after `__init` and N
//! scans (`Run.v0 = -5`, `Run.v1 = TRUE`). [`crate::generate`] writes such
//! programs; a person can too, to pin a result. This oracle compiles the
//! source, runs it the way a host does, reads every variable back through
//! the debug symbols (what the monitor shows) and compares. Any other
//! source passes untouched.

use std::collections::BTreeMap;

use debug_format::VarValue;
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;

use crate::{Finding, session, wasm};

const HEADER: &str = "(* rk-fuzz expects, after ";

struct Expectation {
    scans: u64,
    values: Vec<(String, String)>,
}

impl Expectation {
    fn parse(source: &str) -> Option<Self> {
        let rest = source.strip_prefix(HEADER)?;
        let (scans, rest) = rest.split_once(" scans:")?;
        let (body, _) = rest.split_once("*)")?;
        let values = body
            .lines()
            .filter_map(|line| {
                let (path, value) = line.split_once(" = ")?;
                Some((path.trim().to_string(), value.trim().to_string()))
            })
            .collect();
        Some(Self {
            scans: scans.trim().parse().ok()?,
            values,
        })
    }
}

pub fn check(source: &str) -> Result<(), Finding> {
    let Some(expected) = Expectation::parse(source) else {
        return Ok(());
    };
    let Some((db, file)) = session::load(source) else {
        return Err(Finding::new(
            "semantics-rejected",
            "the program does not parse",
        ));
    };
    // The program is well-typed by construction: a refusal is a bug in
    // the checker, or in the generator, and either wants a look.
    if !session::compiles(&db, file) {
        let errors: Vec<String> = diagnostics_for_file(&db, file)
            .iter()
            .filter(|d| session::is_error(d))
            .map(|d| format!("{:?} {}", d.diagnostic.code, d.diagnostic.message))
            .collect();
        return Err(Finding::new(
            "semantics-rejected",
            format!("`rk check` refuses a well-typed program: {errors:?}"),
        ));
    }
    let mir =
        mir::lower::lower_module::lower_module(&db, semantic_index(&db, file)).map_err(|e| {
            Finding::new(
                "lowering",
                format!("an accepted source failed to lower: {e}"),
            )
        })?;
    let module = wasm_codegen::generate_wasm(&db, &mir).finish();
    let sections = wasm::decode_sections(&module)?;
    let memory = match wasm::memory_after(&module, &sections, expected.scans)? {
        Ok(memory) => memory,
        Err(unit) => {
            return Err(Finding::new(
                "semantics-trapped",
                format!("{unit} stopped the program, which cannot trap"),
            ));
        }
    };

    let actual: BTreeMap<String, String> = sections
        .values(&memory)
        .into_iter()
        .filter_map(|(path, value)| Some((path, shown(&value)?)))
        .collect();
    for (path, value) in &expected.values {
        match actual.get(path) {
            None => {
                return Err(Finding::new(
                    "semantics-symbol",
                    format!("the debug symbols have no `{path}`"),
                ));
            }
            Some(got) if got != value => {
                return Err(Finding::new(
                    "semantics",
                    format!(
                        "after {} scans `{path}` is {got}, not {value}",
                        expected.scans
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// A value as the header writes it.
fn shown(value: &VarValue) -> Option<String> {
    Some(match value {
        VarValue::Bool(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        VarValue::I8(v) => v.to_string(),
        VarValue::I16(v) => v.to_string(),
        VarValue::I32(v) => v.to_string(),
        VarValue::I64(v) => v.to_string(),
        VarValue::U8(v) => v.to_string(),
        VarValue::U16(v) => v.to_string(),
        VarValue::U32(v) => v.to_string(),
        VarValue::U64(v) => v.to_string(),
        _ => return None,
    })
}

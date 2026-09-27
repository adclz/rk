//! Does the program compute what it should?
//!
//! A source whose first line is `(* rk-fuzz expects, after N scans:`
//! names, one per line, the value each variable holds after `__init` and N
//! scans (`Run.v0 = -5`, `Run.v1 = TRUE`). [`crate::generate`] writes such
//! programs; a person can too, to pin a result. This oracle compiles the
//! source, runs it the way a host does, reads every variable back through
//! the debug symbols (what the monitor shows) and compares. With
//! `RK_FUZZ_OPTIMIZE=1` it does it again on the release module after
//! `wasm-opt`, as `rk compile -O` builds it. Any other source passes
//! untouched.

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
    compare(&module, &expected, "")?;

    // `rk compile -O`: the release module, through the CLI's own wasm-opt
    // path (sections re-attached after), must compute the same values.
    // Opt-in: it runs wasm-opt, and the CLI downloads one when none is
    // installed. The level follows the program, so every level gets its
    // turn over a night.
    let optimize = std::env::var_os(OPTIMIZE_ENV).is_some_and(|v| !v.is_empty());
    if optimize && rk::wasm_opt::find(false).is_some() {
        let level = LEVELS[source.len() % LEVELS.len()];
        let release =
            wasm_codegen::generate_wasm_profile(&db, &mir, wasm_codegen::Profile::Release).finish();
        let optimized =
            rk::compiler::optimize_wasm_release(release, level, false).map_err(|e| {
                Finding::new(
                    "optimize",
                    format!("wasm-opt -O{level} did not take the module: {e}"),
                )
            })?;
        wasm::validate(&optimized, &format!("-O{level}"))?;
        compare(&optimized, &expected, &format!(" with -O{level}"))?;
    }
    Ok(())
}

/// Set it to check each program again after `wasm-opt`.
pub const OPTIMIZE_ENV: &str = "RK_FUZZ_OPTIMIZE";

/// The levels `rk compile -O` takes.
const LEVELS: [&str; 7] = ["0", "1", "2", "3", "4", "s", "z"];

/// Run `module` and read every expected path back through its debug
/// symbols. `build` says which module it was, in the finding.
fn compare(module: &[u8], expected: &Expectation, build: &str) -> Result<(), Finding> {
    let sections = wasm::decode_sections(module)?;
    let memory = match wasm::memory_after(module, &sections, expected.scans)? {
        Ok(memory) => memory,
        Err(unit) => {
            return Err(Finding::new(
                "semantics-trapped",
                format!("{unit} stopped the program{build}, which cannot trap"),
            ));
        }
    };
    for (path, value) in &expected.values {
        match sections.value(path, &memory).as_ref().and_then(shown) {
            None => {
                return Err(Finding::new(
                    "semantics-symbol",
                    format!("the debug symbols{build} do not resolve `{path}`"),
                ));
            }
            Some(got) if got != *value => {
                return Err(Finding::new(
                    "semantics",
                    format!(
                        "after {} scans{build}, `{path}` is {got}, not {value}",
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

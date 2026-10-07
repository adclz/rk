// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `stack_size` in `config.toml` sets the size of the stack recursive calls
//! push their frames on. A frame larger than the stack is refused by
//! `rk check` (E1430), and a stack that does not fit in the 4 GiB a module
//! can address stops the build with an error naming the setting, before
//! codegen lays out a module that would fail at run time.

use rk::cli::OutputFormat;
use rk::diagnostics::{DiagnosticReporter, collect_diagnostics};
use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};

/// A recursive function with a frame of 80000 bytes.
const DEEP: &str = "FUNCTION deep : DINT
VAR_INPUT n : DINT; END_VAR
VAR cells : ARRAY[0..19999] OF DINT; END_VAR
    cells[0] := n;
    IF n > 0 THEN deep := deep(n - 1); ELSE deep := cells[0]; END_IF;
END_FUNCTION
";

/// A `config.toml` setting `stack_size` to `size`.
fn config(size: u64) -> String {
    format!("[project]\nname = \"T\"\nversion = \"0.0\"\n\n[settings]\nstack_size = {size}\n")
}

/// Build a workspace holding [`DEEP`], with `stack_size` set to `size`.
fn build(size: u64) -> Result<(Vec<u8>, mir::MirModule), String> {
    let (_ws, root) = temp_workspace(&[("config.toml", &config(size)), ("main.st", DEEP)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    rk::compiler::build_core_quiet(&db, &root, false)
}

/// What `rk check` reports, one line each, for a workspace holding `source`
/// with `stack_size` set to `size`.
fn check(size: u64, source: &str) -> String {
    let (_ws, root) = temp_workspace(&[("config.toml", &config(size)), ("main.st", source)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    let mut out = Vec::new();
    DiagnosticReporter::new(&db, &root)
        .with_format(OutputFormat::Concise)
        .report_files(&collect_diagnostics(&db, false), &mut out);
    String::from_utf8(out).unwrap()
}

/// The pages of memory the module asks its host for.
fn memory_pages(wasm: &[u8]) -> u64 {
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, wasm).expect("a valid module");
    module
        .imports()
        .find_map(|import| match import.ty() {
            wasmtime::ExternType::Memory(memory) => Some(memory.minimum()),
            _ => None,
        })
        .expect("the module imports its memory")
}

#[test]
fn a_stack_size_sets_the_memory_the_module_asks_for() {
    let (wasm, _) = build(1 << 20).expect("a 1 MiB stack fits");
    // 1 MiB is 16 pages of 64 KiB, after the memory before the stack.
    assert!(memory_pages(&wasm) > 16, "{} pages", memory_pages(&wasm));
}

/// A frame larger than the stack is refused by the check the build starts
/// with, at the body that pushes it.
#[test]
fn a_stack_smaller_than_a_frame_is_refused() {
    let error = build(16384).expect_err("the frame does not fit");
    for expected in [
        "[E1430]",
        "each call of 'deep' pushes 80000 bytes",
        "`stack_size` in config.toml makes it 16384 bytes",
        "make `stack_size` 80000 bytes or more",
    ] {
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

/// A frame holds the result, the inputs and locals the body keeps in memory,
/// each on a multiple of 8 bytes: a STRING[10] result takes 16, a STRING[20]
/// input 24, an INT input `REF()` takes 8, three INTs 16. A scalar input, a
/// reference and the frame of a function that does not call itself take no
/// stack.
#[test]
fn a_frame_holds_what_the_body_keeps_in_memory() {
    let source = "FUNCTION f : STRING[10]
VAR_INPUT s : STRING[20]; k : INT; n : INT; END_VAR
VAR a : ARRAY[0..2] OF INT; r : REF_TO INT; END_VAR
    r := REF(k);
    a[0] := n;
    IF n > 0 THEN f := f(s, k, n - 1); END_IF;
END_FUNCTION

FUNCTION g : STRING[200]
VAR big : ARRAY[0..999] OF INT; END_VAR
    g := 'g';
END_FUNCTION
";
    assert_eq!(
        check(63, source),
        "main.st:1:10: error[E1430]: each call of 'f' pushes 64 bytes\n"
    );
    assert_eq!(check(64, source), "");
}

/// A copy of a body with an `ARRAY[*]` input holds the copy of the array it
/// passes on: it is reported where it is created, by the call binding the
/// array or by the call of a body that passes it on.
#[test]
fn a_copy_larger_than_the_stack_is_reported_where_it_is_created() {
    let source = "FUNCTION SumA : DINT
VAR_INPUT a : ARRAY[*] OF DINT; i : DINT; END_VAR
    IF i < 0 THEN SumA := 0; ELSE SumA := a[i] + SumA(a, i - 1); END_IF;
END_FUNCTION

FUNCTION Via : DINT
VAR_INPUT a : ARRAY[*] OF DINT; END_VAR
    Via := SumA(a, 0);
END_FUNCTION

FUNCTION test : DINT
VAR small : ARRAY[0..1] OF DINT; big : ARRAY[0..9999] OF DINT; END_VAR
    test := SumA(small, 1) + SumA(big, 9999) + Via(small) + Via(big);
END_FUNCTION
";
    assert_eq!(
        check(1024, source),
        "main.st:13:30: error[E1430]: each call of the copy of 'SumA' for these arrays pushes 40000 bytes\n\
         main.st:13:61: error[E1430]: each call of the copy of 'SumA' for these arrays pushes 40000 bytes\n"
    );
}

#[test]
fn a_stack_past_the_memory_is_refused() {
    let error = build(4 << 30).expect_err("the stack passes the last address");
    assert!(
        error.contains(
            "`stack_size` in config.toml is 4294967296 bytes, and the memory before the stack"
        ),
        "{error}"
    );
    assert!(
        error.contains("a module addresses at most 4 GiB"),
        "{error}"
    );
}

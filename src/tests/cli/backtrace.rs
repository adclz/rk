// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test`: where a test failed.
//!
//! A failure is reported with the calls in progress, innermost first. A trap
//! brings wasmtime's backtrace. A `RAISE` brings none, and the wrapper of a
//! test catches it inside the module, so the module `rk test` builds tells
//! the host just before each raise, and the host takes the stack there. A
//! failure was placed at the line the test was declared on, whatever it had
//! called.

use crate::tests::codegen::{TestBuild, compile_tests, compile_to_mir_and_wasm_as_built, with_db};
use debug_format::test_report::TestRecord;
use rstest::rstest;
use std::time::Duration;

/// The 1-based line of the one line of `source` that carries `marker`.
fn line_of(source: &str, marker: &str) -> u32 {
    let mut lines = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(marker));
    let (index, _) = lines
        .next()
        .unwrap_or_else(|| panic!("no line marked {marker}"));
    assert!(lines.next().is_none(), "{marker} marks two lines");
    index as u32 + 1
}

/// A record's backtrace as `(function, line)` pairs.
fn calls(record: &TestRecord) -> Vec<(String, u32)> {
    record
        .backtrace
        .iter()
        .map(|frame| {
            assert_eq!(frame.file.as_deref(), Some("file:///test0.st"));
            (frame.function.clone(), frame.line.expect("a line"))
        })
        .collect()
}

/// The tests of `component`, run as `rk test` runs them.
fn run(component: &[u8], budget: Option<Duration>) -> Vec<TestRecord> {
    rk::test_host::run_each(component, None, budget, |_| {}).expect("the tests run")
}

/// The tests of `build`'s component, run as `rk test` runs them.
fn run_build(build: &TestBuild, budget: Option<Duration>) -> Vec<TestRecord> {
    run(&build.component, budget)
}

fn only(records: Vec<TestRecord>) -> TestRecord {
    assert_eq!(records.len(), 1, "one test");
    records.into_iter().next().unwrap()
}

/// `wasm` as an optimizer leaves it: without the tables a failure is placed
/// with.
fn without_line_tables(wasm: &[u8]) -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        let payload = payload.expect("a valid module");
        if let wasmparser::Payload::CustomSection(section) = &payload
            && matches!(
                section.name(),
                "debug-lines" | "debug-functions" | "debug-locals"
            )
        {
            continue;
        }
        if let Some((id, range)) = payload.as_section() {
            module.section(&wasm_encoder::RawSection {
                id,
                data: &wasm[range],
            });
        }
    }
    module.finish()
}

/// A RAISE the test's wrapper catches, three calls down: each caller is on
/// the line of its call. `Fail();` is the first instruction of its statement
/// and `m.Start();` the last of its own, with a statement on either side:
/// a call's position is its own instruction, not the one it returns to, and
/// stepping off it either way names a neighbor.
const RAISED: &str = r#"
        FUNCTION Fail
            __RAISE('boom');                // raise
        END_FUNCTION

        FUNCTION_BLOCK Motor
        VAR n : INT; END_VAR
            METHOD PUBLIC Start
                n := n + 1;
                Fail();                     // in the method
                n := n + 1;
            END_METHOD
        END_FUNCTION_BLOCK

        {test}
        FUNCTION test_raise
        VAR m : Motor; x : INT; END_VAR
            x := 1;
            m.Start();                      // in the test
            x := 2;
        END_FUNCTION
    "#;

#[rstest]
fn a_raise_is_placed_with_its_callers(mut with_db: db::RootDatabase) {
    let record = only(run_build(&compile_tests(&mut with_db, RAISED), None));
    assert_eq!(record.reason.as_deref(), Some("boom"));
    assert_eq!(
        calls(&record),
        [
            ("Fail".to_string(), line_of(RAISED, "// raise")),
            (
                "Motor.Start".to_string(),
                line_of(RAISED, "// in the method")
            ),
            ("test_raise".to_string(), line_of(RAISED, "// in the test")),
        ]
    );
    let raise = &record.backtrace[0];
    assert_eq!(raise.column, Some(13), "the statement's first column");
    assert_eq!(
        record.line,
        Some(line_of(RAISED, "FUNCTION test_raise")),
        "the record still says where the test is declared"
    );
}

/// The checks the compiler inserts raise from a bundled function with no
/// name in the tables: the innermost call shown is the statement that
/// failed the check, not the check.
#[rstest]
#[case::subrange("x := v;", "value out of subrange bounds")]
#[case::subscript("x := a[v];", "array index out of bounds")]
fn a_failed_check_is_placed_at_its_statement(
    mut with_db: db::RootDatabase,
    #[case] statement: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"
        FUNCTION Checked : INT
        VAR_INPUT v : INT; END_VAR
        VAR
            x : INT (0..10);
            a : ARRAY[0..3] OF INT (0..10);
        END_VAR
            {statement}                     // checked
            Checked := x;
        END_FUNCTION

        {{test}}
        FUNCTION test_check
        VAR r : INT; END_VAR
            r := Checked(50);               // call
        END_FUNCTION
    "#
    );
    let record = only(run_build(&compile_tests(&mut with_db, &source), None));
    assert_eq!(record.reason.as_deref(), Some(message));
    assert_eq!(
        calls(&record),
        [
            ("Checked".to_string(), line_of(&source, "// checked")),
            ("test_check".to_string(), line_of(&source, "// call")),
        ]
    );
}

/// What an import is passed is checked by a wrapper the tables do not name,
/// before the host is called: a failed check is placed at the call, as an
/// inserted check is, and the host is never reached.
#[rstest]
fn a_string_an_import_cannot_take_is_placed_at_the_call(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'host' 'show'}
        FUNCTION Show : INT
        VAR_INPUT line : STRING; END_VAR
        END_FUNCTION

        {test}
        FUNCTION test_show
        VAR r : INT; END_VAR
            r := Show('caf$E9');            // call
        END_FUNCTION
    "#;
    let record = only(run_build(&compile_tests(&mut with_db, source), None));
    assert_eq!(
        record.reason.as_deref(),
        Some("a STRING passed to the host is not UTF-8")
    );
    assert_eq!(
        calls(&record),
        [("test_show".to_string(), line_of(source, "// call"))]
    );
}

/// A trap is not an exception: it comes back with wasmtime's backtrace, read
/// as it came. Its function indices count the module's imports first, which
/// the tables do not: one `{extern}` here shifts them. Its offsets count
/// from the start of the component, where the module is not: the host
/// brings them back to the module's.
const TRAPPED: &str = r#"
        {extern 'host' 'absent'}
        FUNCTION Absent : INT
        VAR_INPUT x : INT; END_VAR
        END_FUNCTION

        FUNCTION Truncate : DINT
        VAR_INPUT x : REAL; END_VAR
            {wasm Truncate 'trunc_f32_s' (params x) (result Truncate)}   // truncates
        END_FUNCTION

        FUNCTION Spin : INT
        VAR n : INT; END_VAR
            WHILE TRUE DO
                n := n + 1;
            END_WHILE;
        END_FUNCTION

        {test}
        FUNCTION test_truncate
        VAR r : DINT; END_VAR
            r := Truncate(1.0E30);          // call to Truncate
        END_FUNCTION

        {test}
        FUNCTION test_import
        VAR r : INT; END_VAR
            r := Absent(1);                 // call to Absent
        END_FUNCTION

        {test}
        FUNCTION test_spin
        VAR r : INT; END_VAR
            r := Spin();                    // call to Spin
        END_FUNCTION
    "#;

/// What the three tests of [`TRAPPED`] must report, whichever way each was
/// placed. The loop is stopped wherever it was: only its function is fixed.
fn assert_traps_are_placed(records: &[TestRecord]) {
    let by_name = |name: &str| {
        records
            .iter()
            .find(|record| record.name == name)
            .unwrap_or_else(|| panic!("no record for {name}"))
    };
    assert_eq!(
        calls(by_name("test_truncate")),
        [
            ("Truncate".to_string(), line_of(TRAPPED, "// truncates")),
            (
                "test_truncate".to_string(),
                line_of(TRAPPED, "// call to Truncate")
            ),
        ]
    );
    assert_eq!(
        calls(by_name("test_import")),
        [(
            "test_import".to_string(),
            line_of(TRAPPED, "// call to Absent")
        )],
        "the import has no body, and no frame"
    );
    let spin = by_name("test_spin");
    assert!(
        spin.reason
            .as_deref()
            .is_some_and(|reason| reason.contains("the watchdog stopped it")),
        "got {:?}",
        spin.reason
    );
    let spin = calls(spin);
    assert_eq!(spin.len(), 2, "got {spin:?}");
    assert_eq!(spin[0].0, "Spin");
    assert_eq!(
        spin[1],
        ("test_spin".to_string(), line_of(TRAPPED, "// call to Spin")),
        "a statement that starts with its call"
    );
}

#[rstest]
fn a_trap_is_placed_from_its_backtrace(mut with_db: db::RootDatabase) {
    let build = compile_tests(&mut with_db, TRAPPED);
    assert_traps_are_placed(&run_build(&build, Some(Duration::from_millis(200))));
}

/// An optimized module has lost its line tables: a failure, a trap like a
/// raise, is reported with its message and the line its test is declared
/// on, and without the calls, which nothing names.
#[rstest]
#[case::trap(TRAPPED)]
#[case::raise(RAISED)]
fn a_failure_of_an_optimized_module_has_no_calls(
    mut with_db: db::RootDatabase,
    #[case] source: &str,
) {
    let build = compile_tests(&mut with_db, source);
    let optimized = build.wrap(&without_line_tables(&build.core));
    let records = run(&optimized, Some(Duration::from_millis(200)));
    assert!(!records.is_empty());
    for record in &records {
        assert!(record.reason.is_some(), "{record:?}");
        assert!(record.backtrace.is_empty(), "{record:?}");
        assert_eq!(
            record.line,
            Some(line_of(source, &format!("FUNCTION {}", record.name))),
            "{record:?}"
        );
    }
}

/// A test that passes has no backtrace.
#[rstest]
fn a_pass_has_no_backtrace(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_fine
        END_FUNCTION
    "#;
    let record = only(run_build(&compile_tests(&mut with_db, source), None));
    assert!(record.passed());
    assert!(record.backtrace.is_empty());
}

/// The backtrace is in the record's JSON, and a record without one reads as
/// before: a report from an older runner still parses.
#[rstest]
fn a_backtrace_is_in_the_report(mut with_db: db::RootDatabase) {
    let record = only(run_build(&compile_tests(&mut with_db, RAISED), None));
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["backtrace"][0]["function"], "Fail");
    assert_eq!(
        json["backtrace"][0]["line"],
        line_of(RAISED, "// raise"),
        "1-based, as the record's own line"
    );
    let older = r#"{"name":"t","status":"fail","reason":"boom","duration_us":1}"#;
    let read: TestRecord = serde_json::from_str(older).unwrap();
    assert!(read.backtrace.is_empty());
    assert!(
        !serde_json::to_string(&read).unwrap().contains("backtrace"),
        "an empty backtrace is left out"
    );
}

/// A caller is on the line of its call, whatever follows the call: nothing
/// (`Void();` and `Arg(5);` end their statement, with another after it) or a
/// store (`n := Res();`), and whether the callee raised, trapped or was
/// stopped. A caller's offset is its call instruction's, in the stack the
/// host takes at a raise as in a trap's backtrace, not where it resumes,
/// which would place the two calls that end their statement on the line
/// after.
#[rstest]
#[case::raise("__RAISE('boom');")]
#[case::stopped("WHILE TRUE DO k := k + 1; END_WHILE;")]
#[case::trap("{wasm k 'trunc_f32_s' (params r) (result k)}")]
fn a_caller_is_on_the_line_of_its_call(mut with_db: db::RootDatabase, #[case] fault: &str) {
    let source = format!(
        r#"
        FUNCTION Void
        VAR k : INT; r : REAL := 1.0E30; END_VAR
            {fault}
        END_FUNCTION

        FUNCTION Arg
        VAR_INPUT a : INT; END_VAR
        VAR k : INT; r : REAL := 1.0E30; END_VAR
            {fault}
        END_FUNCTION

        FUNCTION Res : INT
        VAR k : INT; r : REAL := 1.0E30; END_VAR
            {fault}
        END_FUNCTION

        FUNCTION CallVoid
        VAR n : INT; END_VAR
            n := 1;
            Void();                         // calls Void
            n := 2;
        END_FUNCTION

        FUNCTION CallArg
        VAR n : INT; END_VAR
            n := 1;
            Arg(5);                         // calls Arg
            n := 2;
        END_FUNCTION

        FUNCTION CallRes
        VAR n : INT; END_VAR
            n := 1;
            n := Res();                     // calls Res
            n := 2;
        END_FUNCTION

        {{test}}
        FUNCTION t_void
            CallVoid();
        END_FUNCTION

        {{test}}
        FUNCTION t_arg
            CallArg();
        END_FUNCTION

        {{test}}
        FUNCTION t_res
            CallRes();
        END_FUNCTION
    "#
    );
    let records = run_build(
        &compile_tests(&mut with_db, &source),
        Some(Duration::from_millis(150)),
    );
    for (test, caller, marker) in [
        ("t_void", "CallVoid", "// calls Void"),
        ("t_arg", "CallArg", "// calls Arg"),
        ("t_res", "CallRes", "// calls Res"),
    ] {
        let record = records
            .iter()
            .find(|record| record.name == test)
            .unwrap_or_else(|| panic!("no record for {test}"));
        let calls = calls(record);
        assert_eq!(calls.len(), 3, "{test}: {calls:?}");
        assert_eq!(
            calls[1],
            (caller.to_string(), line_of(&source, marker)),
            "{test}"
        );
    }
}

/// A block's body is named by its block: the tables call it
/// `Ramp$__body__`, a name no source has.
#[rstest]
fn a_block_s_body_is_named_by_its_block(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Ramp
        VAR level : INT (0..10); END_VAR
        VAR_INPUT step : INT; END_VAR
            level := step;                  // stores
        END_FUNCTION_BLOCK

        {test}
        FUNCTION test_ramp
        VAR r : Ramp; END_VAR
            r(step := 50);                  // calls the block
        END_FUNCTION
    "#;
    assert_eq!(
        calls(&only(run_build(&compile_tests(&mut with_db, source), None))),
        [
            ("Ramp".to_string(), line_of(source, "// stores")),
            (
                "test_ramp".to_string(),
                line_of(source, "// calls the block")
            ),
        ]
    );
}

/// A raise the test's own `__CATCH` takes is not where the test failed:
/// the failure is placed at the last throw, the one nothing caught.
#[rstest]
fn a_caught_raise_is_not_where_the_test_failed(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_twice
        VAR e : STRING; END_VAR
            __TRY
                __RAISE('handled');             // caught
            __CATCH(e)
                e := '';
            __END_TRY;
            __RAISE('not handled');             // failed
        END_FUNCTION
    "#;
    let record = only(run_build(&compile_tests(&mut with_db, source), None));
    assert_eq!(record.reason.as_deref(), Some("not handled"));
    assert_eq!(
        calls(&record),
        [("test_twice".to_string(), line_of(source, "// failed"))]
    );
}

/// A raise that goes through a `__FINALLY` keeps its place: the finally
/// throws it on, which says nothing to the host. One raised again in a
/// `__CATCH` is a raise of its own, placed where it is.
#[rstest]
fn a_raise_keeps_its_place_through_a_finally(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_finally
        VAR n : INT; END_VAR
            __TRY
                __RAISE('through');             // the raise
            __FINALLY
                n := 1;
            __END_TRY;
        END_FUNCTION

        {test}
        FUNCTION test_caught_and_raised
        VAR e : STRING; END_VAR
            __TRY
                __RAISE('first');
            __CATCH(e)
                __RAISE('second');              // raised again
            __END_TRY;
        END_FUNCTION
    "#;
    let records = run_build(&compile_tests(&mut with_db, source), None);
    let by_name = |name: &str| {
        records
            .iter()
            .find(|record| record.name == name)
            .unwrap_or_else(|| panic!("no record for {name}"))
    };
    let finally = by_name("test_finally");
    assert_eq!(finally.reason.as_deref(), Some("through"));
    assert_eq!(
        calls(finally),
        [("test_finally".to_string(), line_of(source, "// the raise"))]
    );
    let again = by_name("test_caught_and_raised");
    assert_eq!(again.reason.as_deref(), Some("second"));
    assert_eq!(
        calls(again),
        [(
            "test_caught_and_raised".to_string(),
            line_of(source, "// raised again")
        )]
    );
}

/// The functions `wasm` imports, by module and name.
fn imports(wasm: &[u8]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::ImportSection(reader)) = payload {
            for import in reader.into_imports() {
                let import = import.expect("an import");
                found.push((import.module.to_string(), import.name.to_string()));
            }
        }
    }
    found
}

/// The module `rk test` builds says where it raises: it imports
/// `rk:test/raise`'s `raised`, which its world names. A module `rk compile`
/// builds does not, debug or release: the two profiles differ in their
/// custom sections only.
#[rstest]
fn only_the_module_rk_test_builds_says_where_it_raises(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_raise
            __RAISE('boom');
        END_FUNCTION
    "#;
    let hook = (
        debug_format::wit::TEST_INTERFACE.to_string(),
        mir::raise_hook::FUNCTION.to_string(),
    );
    let build = compile_tests(&mut with_db, source);
    assert!(
        imports(&build.core).contains(&hook),
        "{:?}",
        imports(&build.core)
    );
    let interfaces = rk::component::interfaces(&with_db, &build.mir).expect("typed interfaces");
    let world = rk::component::wrap(&build.core, "plant", &interfaces)
        .expect("the module wraps")
        .world;
    assert!(
        world.contains(&format!("import {};", debug_format::wit::TEST_INTERFACE)),
        "{world}"
    );

    let mut plain = db::RootDatabase::default();
    let (mir, debug) = compile_to_mir_and_wasm_as_built(&mut plain, source);
    let release =
        wasm_codegen::generate_wasm_profile(&plain, &mir, wasm_codegen::Profile::Release).finish();
    for (profile, wasm) in [("debug", debug), ("release", release)] {
        assert!(!imports(&wasm).contains(&hook), "{profile}");
    }
}

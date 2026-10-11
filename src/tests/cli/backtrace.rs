// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test`: where a test failed.
//!
//! A failure is reported with the calls in progress, innermost first. A trap
//! brings wasmtime's backtrace. A `RAISE` brings none, and the wrapper of a
//! test catches it inside the module, so the test runs once more in a debug
//! store that is called at the throw. A failure was placed at the line the
//! test was declared on, whatever it had called.

use crate::tests::codegen::{compile_to_wasm, with_db};
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

fn run(wasm: &[u8], budget: Option<Duration>) -> Vec<TestRecord> {
    rk::test_host::run_each(wasm, None, budget, |_| {}).expect("the tests run")
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
    let wasm = compile_to_wasm(&mut with_db, RAISED);
    let record = only(run(&wasm, None));
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let record = only(run(&wasm, None));
    assert_eq!(record.reason.as_deref(), Some(message));
    assert_eq!(
        calls(&record),
        [
            ("Checked".to_string(), line_of(&source, "// checked")),
            ("test_check".to_string(), line_of(&source, "// call")),
        ]
    );
}

/// A trap is not an exception: it comes back with wasmtime's backtrace, read
/// as it came, with no second run. Its function indices count the module's
/// imports first, which the tables do not: one `{extern}` here shifts them.
const TRAPPED: &str = r#"
        {extern 'host' 'absent'}
        FUNCTION Absent : INT
        VAR_INPUT x : INT; END_VAR
        END_FUNCTION

        FUNCTION Divide : INT
        VAR_INPUT n : INT; d : INT; END_VAR
            Divide := n / d;                // divides
        END_FUNCTION

        FUNCTION Spin : INT
        VAR n : INT; END_VAR
            WHILE TRUE DO
                n := n + 1;
            END_WHILE;
        END_FUNCTION

        {test}
        FUNCTION test_divide
        VAR r : INT; END_VAR
            r := Divide(1, 0);              // call to Divide
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
        calls(by_name("test_divide")),
        [
            ("Divide".to_string(), line_of(TRAPPED, "// divides")),
            (
                "test_divide".to_string(),
                line_of(TRAPPED, "// call to Divide")
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
    let wasm = compile_to_wasm(&mut with_db, TRAPPED);
    let records = run(&wasm, Some(Duration::from_millis(200)));
    assert_traps_are_placed(&records);
}

/// An optimized module has lost its line tables: a failure is placed by
/// running its test on the build it was made from, a trap like a raise.
#[rstest]
fn a_failure_of_an_optimized_module_is_placed_on_the_unoptimized_one(
    mut with_db: db::RootDatabase,
) {
    let wasm = compile_to_wasm(&mut with_db, TRAPPED);
    let optimized = without_line_tables(&wasm);

    let alone = run(&optimized, Some(Duration::from_millis(200)));
    assert_eq!(alone.len(), 3);
    assert!(
        alone.iter().all(|record| record.backtrace.is_empty()),
        "nothing to place a failure with"
    );

    let located = rk::test_host::run_each_located(
        &optimized,
        Some(&wasm),
        None,
        Some(Duration::from_millis(200)),
        |_| {},
    )
    .expect("the tests run");
    assert_traps_are_placed(&located);
}

/// The same for a raise: three calls, from the unoptimized build.
#[rstest]
fn a_raise_of_an_optimized_module_is_placed_too(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, RAISED);
    let located = rk::test_host::run_each_located(
        &without_line_tables(&wasm),
        Some(&wasm),
        None,
        None,
        |_| {},
    )
    .expect("the tests run");
    assert_eq!(
        calls(&only(located)),
        [
            ("Fail".to_string(), line_of(RAISED, "// raise")),
            (
                "Motor.Start".to_string(),
                line_of(RAISED, "// in the method")
            ),
            ("test_raise".to_string(), line_of(RAISED, "// in the test")),
        ]
    );
}

/// A test that passes has no backtrace, and is not run twice.
#[rstest]
fn a_pass_has_no_backtrace(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_fine
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let record = only(run(&wasm, None));
    assert!(record.passed());
    assert!(record.backtrace.is_empty());
}

/// The backtrace is in the record's JSON, and a record without one reads as
/// before: a report from an older runner still parses.
#[rstest]
fn a_backtrace_is_in_the_report(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, RAISED);
    let record = only(run(&wasm, None));
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
/// stopped. Each is placed twice, as the module ran and by running the test
/// again on the build with the tables.
///
/// The debug frames wasmtime hands a handler do not hold: a caller's pc
/// there is where it resumes, so the two calls that end their statement
/// were placed on the line after. The handler takes a backtrace instead,
/// the one a trap comes with.
#[rstest]
#[case::raise("__RAISE('boom');")]
#[case::stopped("WHILE TRUE DO k := k + 1; END_WHILE;")]
#[case::trap("k := 1 / z;")]
fn a_caller_is_on_the_line_of_its_call(mut with_db: db::RootDatabase, #[case] fault: &str) {
    let source = format!(
        r#"
        FUNCTION Void
        VAR k : INT; z : INT; END_VAR
            {fault}
        END_FUNCTION

        FUNCTION Arg
        VAR_INPUT a : INT; END_VAR
        VAR k : INT; z : INT; END_VAR
            {fault}
        END_FUNCTION

        FUNCTION Res : INT
        VAR k : INT; z : INT; END_VAR
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
    let wasm = compile_to_wasm(&mut with_db, &source);
    let budget = Some(Duration::from_millis(150));
    let as_ran = run(&wasm, budget);
    let again = rk::test_host::run_each_located(
        &without_line_tables(&wasm),
        Some(&wasm),
        None,
        budget,
        |_| {},
    )
    .expect("the tests run");
    for (path, records) in [("as it ran", as_ran), ("run again", again)] {
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
            assert_eq!(calls.len(), 3, "{path}, {test}: {calls:?}");
            assert_eq!(
                calls[1],
                (caller.to_string(), line_of(&source, marker)),
                "{path}, {test}"
            );
        }
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
    let wasm = compile_to_wasm(&mut with_db, source);
    assert_eq!(
        calls(&only(run(&wasm, None))),
        [
            ("Ramp".to_string(), line_of(source, "// stores")),
            (
                "test_ramp".to_string(),
                line_of(source, "// calls the block")
            ),
        ]
    );
}

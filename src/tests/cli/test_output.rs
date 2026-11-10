// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test`: what a test wrote to the standard output and error, kept and
//! shown with the test when it fails.

use crate::tests::codegen::{compile_to_mir_and_wasm_as_built, with_db};
use debug_format::test_report::TestRecord;
use rk::test_host::Target;
use rstest::rstest;

/// The two writes, as `Std.Console` declares them.
const WRITES: &str = r#"
    FUNCTION Say : BOOL
    VAR_INPUT IN : STRING[4096]; END_VAR
        {wasm 'rk.stdout_write' (params IN) (result Say)}
    END_FUNCTION

    FUNCTION Warn : BOOL
    VAR_INPUT IN : STRING[4096]; END_VAR
        {wasm 'rk.stderr_write' (params IN) (result Warn)}
    END_FUNCTION
"#;

/// The records of `source`'s tests, run on the component as `rk test` runs
/// them, and on the core module alone as the codegen tests do.
fn run(db: &mut db::RootDatabase, source: &str) -> [Vec<TestRecord>; 2] {
    let (mir, core) = compile_to_mir_and_wasm_as_built(db, source);
    let interfaces = rk::component::interfaces(db, &mir).expect("typed interfaces");
    let component = rk::component::wrap(&core, "plant", &interfaces).expect("the module wraps");
    [Target::Component(&component.bytes), Target::Core(&core)]
        .map(|target| rk::test_host::run_each(target, None, None, |_| {}).expect("the tests run"))
}

fn record<'a>(records: &'a [TestRecord], name: &str) -> &'a TestRecord {
    records
        .iter()
        .find(|record| record.name == name)
        .unwrap_or_else(|| panic!("no record for {name}"))
}

/// A failed test's record carries what it wrote, both outputs in the order
/// written, once: the run that places the failure writes nowhere. A test
/// that passes keeps quiet, whatever it wrote.
#[rstest]
fn a_failure_carries_what_the_test_wrote(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{WRITES}
        {{test}}
        FUNCTION test_fails
        VAR written : BOOL; END_VAR
            written := Say('reading the sensor$N');
            written := Warn('pressure is high$N');
            written := Say('giving up$N');
            __RAISE('pressure over 3 bar');     // raises
        END_FUNCTION

        {{test}}
        FUNCTION test_passes
        VAR written : BOOL; END_VAR
            written := Say('all good$N');
        END_FUNCTION

        {{test}}
        FUNCTION test_fails_quietly
            __RAISE('nothing written');
        END_FUNCTION
    "#
    );
    for (target, records) in ["the component", "the core module"]
        .into_iter()
        .zip(run(&mut with_db, &source))
    {
        let failed = record(&records, "test_fails");
        assert_eq!(
            failed.reason.as_deref(),
            Some("pressure over 3 bar"),
            "{target}"
        );
        assert_eq!(
            failed.output.as_deref(),
            Some("reading the sensor\npressure is high\ngiving up\n"),
            "{target}"
        );
        // Placed at the raise, past the three writes.
        let raise = source
            .lines()
            .position(|line| line.contains("// raises"))
            .expect("the marked line") as u32
            + 1;
        assert_eq!(
            failed.backtrace.first().and_then(|frame| frame.line),
            Some(raise),
            "{target}"
        );
        assert_eq!(record(&records, "test_passes").output, None, "{target}");
        assert_eq!(
            record(&records, "test_fails_quietly").output,
            None,
            "{target}"
        );
    }
}

/// A test keeps the first 64 KiB it writes, and a last line says how many
/// bytes came after: a test writing in a loop does not fill the report.
#[rstest]
fn a_test_keeps_the_first_64_kib_it_writes(mut with_db: db::RootDatabase) {
    let source = format!(
        r#"{WRITES}
        {{test}}
        FUNCTION test_floods
        VAR i : INT; written : BOOL; END_VAR
            FOR i := 1 TO 700 DO
                written := Say('{}$N');
            END_FOR;
            __RAISE('flooded');
        END_FUNCTION
    "#,
        "x".repeat(99)
    );
    for records in run(&mut with_db, &source) {
        let output = record(&records, "test_floods")
            .output
            .as_deref()
            .expect("an output");
        let (kept, note) = output.rsplit_once('\n').expect("a last line");
        assert_eq!(kept.len(), rk::test_host::OUTPUT_LIMIT);
        assert_eq!(
            note,
            format!(
                "[{} more bytes not kept]",
                700 * 100 - rk::test_host::OUTPUT_LIMIT
            )
        );
    }
}

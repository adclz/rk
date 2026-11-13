// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk test` runs a test FUNCTION_BLOCK cycle by cycle, on an instance of
//! its own, until a cycle leaves its DONE TRUE. A raise fails it in the
//! cycle that raised, and so does its budget running out before DONE.

use crate::tests::codegen::{STD_CONSOLE, compile_tests, compile_tests_with_libraries, with_db};
use debug_format::test_report::{Status, TestRecord};
use rstest::rstest;
use std::time::Duration;

/// The tests of `component`, run as `rk test` runs them.
fn run(component: &[u8], budget: Option<Duration>) -> Vec<TestRecord> {
    rk::test_host::run_each(component, None, budget, |_| {}).expect("the tests run")
}

/// Each cycle finds the instance as the last one left it, started once from
/// its start values, and the runner stops at the cycle that leaves DONE
/// TRUE. A test FUNCTION beside it runs once, as it always has.
#[rstest]
fn a_test_block_runs_cycle_by_cycle_until_done(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION_BLOCK test_counts_three_cycles
        VAR_OUTPUT DONE : BOOL; END_VAR
        VAR count : INT := 10; cycles : INT; END_VAR
            cycles := cycles + 1;
            IF count <> 9 + cycles THEN
                __RAISE('the instance did not start at 10 once, or did not keep');
            END_IF;
            IF cycles > 3 THEN
                __RAISE('a cycle ran past DONE');
            END_IF;
            count := count + 1;
            DONE := cycles = 3;
        END_FUNCTION_BLOCK

        {test}
        FUNCTION test_a_function_runs_once
        END_FUNCTION
    "#;
    let build = compile_tests(&mut with_db, source);
    let entries: Vec<_> = rk::test_host::discover(&build.core)
        .into_iter()
        .map(|entry| (entry.path, entry.line))
        .collect();
    assert_eq!(
        entries,
        [
            ("test_a_function_runs_once".to_string(), 18),
            ("test_counts_three_cycles".to_string(), 3),
        ],
        "a test block is listed where it is declared"
    );
    let outcomes: Vec<_> = run(&build.component, None)
        .into_iter()
        .map(|record| (record.name, record.status, record.reason))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("test_a_function_runs_once".to_string(), Status::Pass, None),
            ("test_counts_three_cycles".to_string(), Status::Pass, None),
        ]
    );
}

/// A cycle that raises fails the test there, after cycles that left DONE
/// FALSE beside outputs that are not: the failure is placed at the raise,
/// in the block's body, as a test FUNCTION's is in the FUNCTION.
#[rstest]
fn a_test_block_fails_in_the_cycle_that_raises(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION_BLOCK test_raises_in_its_third_cycle
        VAR_OUTPUT before : INT; DONE : BOOL; after : INT; END_VAR
        VAR cycles : INT; END_VAR
            cycles := cycles + 1;
            before := -1;
            after := -1;
            IF cycles = 3 THEN
                __RAISE('third cycle');
            END_IF;
        END_FUNCTION_BLOCK
    "#;
    let build = compile_tests(&mut with_db, source);
    let records = run(&build.component, None);
    let [record] = records.as_slice() else {
        panic!("one test: {records:?}");
    };
    assert_eq!(record.status, Status::Fail);
    assert_eq!(record.reason.as_deref(), Some("third cycle"));
    let calls: Vec<_> = record
        .backtrace
        .iter()
        .map(|frame| (frame.function.as_str(), frame.line))
        .collect();
    assert_eq!(calls, [("test_raises_in_its_third_cycle", Some(10))]);
}

/// A block that never sets DONE fails once its budget is spent, each cycle
/// having returned: the report says how many ran.
#[rstest]
fn a_test_block_that_never_sets_done_fails_when_its_budget_is_spent(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION_BLOCK test_never_done
        VAR_OUTPUT DONE : BOOL; END_VAR
        END_FUNCTION_BLOCK
    "#;
    let build = compile_tests(&mut with_db, source);
    let records = run(&build.component, Some(Duration::from_millis(100)));
    let [record] = records.as_slice() else {
        panic!("one test: {records:?}");
    };
    assert_eq!(record.status, Status::Fail);
    let reason = record.reason.as_deref().unwrap_or_default();
    let cycles = reason
        .strip_prefix("the watchdog stopped it after 100ms: DONE was still FALSE after ")
        .and_then(|rest| rest.strip_suffix(" cycles"))
        .and_then(|count| count.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("the reason: {reason}"));
    assert!(cycles > 1, "{reason}");
    assert!(record.backtrace.is_empty(), "no call was in progress");
}

/// A block that waits on the host's clock, through WASI's `wait-for`, ends:
/// the runtime the host waits on has a turn between cycles, which a cycle
/// that waits on nothing never gives it.
#[rstest]
fn a_test_block_waiting_on_the_host_s_clock_ends(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
        FUNCTION_BLOCK WAIT_FOR
        VAR_INPUT how_long : ULINT; END_VAR
        VAR_OUTPUT DONE : BOOL; END_VAR
        END_FUNCTION_BLOCK

        {test}
        FUNCTION_BLOCK test_waits_a_millisecond
        VAR_OUTPUT DONE : BOOL; END_VAR
        VAR wait : WAIT_FOR; END_VAR
            wait(how_long := ULINT#1000000);
            DONE := wait.DONE;
        END_FUNCTION_BLOCK
    "#;
    let build = compile_tests(&mut with_db, source);
    let records = run(&build.component, Some(Duration::from_secs(5)));
    let [record] = records.as_slice() else {
        panic!("one test: {records:?}");
    };
    assert_eq!(record.status, Status::Pass, "{:?}", record.reason);
}

/// A test block holds what a test FUNCTION cannot, a `STDOUT`, whose write
/// the host takes across cycles; a failure shows what it wrote.
#[rstest]
fn a_test_block_writes_through_its_own_stdout(mut with_db: db::RootDatabase) {
    let source = r#"
        USING Std.Console;

        {test}
        FUNCTION_BLOCK test_writes_then_fails
        VAR_OUTPUT DONE : BOOL; END_VAR
        VAR out : STDOUT; END_VAR
            out(REQ := TRUE, IN := 'written by a block$N');
            IF out.DONE THEN
                __RAISE('failed after writing');
            END_IF;
        END_FUNCTION_BLOCK
    "#;
    let build = compile_tests_with_libraries(&mut with_db, STD_CONSOLE, source);
    let records = run(&build.component, None);
    let [record] = records.as_slice() else {
        panic!("one test: {records:?}");
    };
    assert_eq!(record.reason.as_deref(), Some("failed after writing"));
    assert_eq!(record.output.as_deref(), Some("written by a block\n"));
}

/// A test block is exported as `Block$__test__`, which no POU can be named:
/// one named like an export of the module's own, `memory`, runs.
#[rstest]
fn a_test_block_named_like_a_module_export_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION_BLOCK memory
        VAR_OUTPUT DONE : BOOL; END_VAR
            DONE := TRUE;
        END_FUNCTION_BLOCK
    "#;
    let build = compile_tests(&mut with_db, source);
    let outcomes: Vec<_> = run(&build.component, None)
        .into_iter()
        .map(|record| (record.name, record.status))
        .collect();
    assert_eq!(outcomes, [("memory".to_string(), Status::Pass)]);
}

//! End-to-end: compile IEC to a core module and run its `{test}` functions
//! through the runtime — the same loader a plant uses, no component wrapper.

use rstest::rstest;

use super::with_db;

#[rstest]
fn test_e2e_runtime(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION double : INT
VAR_INPUT x : INT; END_VAR
    double := x + x;
END_FUNCTION

FUNCTION add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    add := a + b;
END_FUNCTION

FUNCTION assert_eq_int
VAR_INPUT value : INT; target : INT; END_VAR
    IF value <> target THEN
        __RAISE('');
    END_IF;
END_FUNCTION

{test}
FUNCTION test_double
    assert_eq_int(value := double(x := 21), target := 42);
END_FUNCTION

{test}
FUNCTION test_add
    assert_eq_int(value := add(a := 10, b := 32), target := 42);
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = wasm_codegen::generate_wasm(&with_db, &mir_module).finish();

    // The manifest is embedded in the component as a custom section — no sidecar.
    let failures = rk::test_runner::run_tests(&core_bytes, None, rk::cli::OutputFormat::Full);
    assert_eq!(failures, 0, "Expected all e2e tests to pass");
}

/// A `__RAISE` inside a `{test}` function propagates as a wasm exception
/// to the test runner, which counts it as a failure (rather than crashing
/// or returning success). Sanity-checks the no-more-host-import wiring.
#[rstest]
fn test_e2e_failing_test_is_counted_as_failure(mut with_db: db::RootDatabase) {
    let source = r#"
{test}
FUNCTION test_intentional_failure
    __RAISE('this test is meant to fail');
END_FUNCTION

{test}
FUNCTION test_passes
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = wasm_codegen::generate_wasm(&with_db, &mir_module).finish();

    // The manifest is embedded in the component as a custom section — no sidecar.
    let failures = rk::test_runner::run_tests(&core_bytes, None, rk::cli::OutputFormat::Full);
    assert_eq!(
        failures, 1,
        "Expected exactly one failing test (`__RAISE` propagated as failure)"
    );
}


/// A `__RAISE` message reaches the report intact.
///
/// The message is written into the test's 12-byte result area as a pointer and
/// length; the runtime reads it out of the same linear memory it owns. That
/// decode used to be the component wrapper's job, and it is the reason a
/// failing test can say what went wrong rather than only that it did.
#[rstest]
fn a_raise_message_survives_into_the_test_result(mut with_db: db::RootDatabase) {
    let source = r#"
{test}
FUNCTION test_fails_with_message
    __RAISE('expected-failure-from-test-fixture');
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = wasm_codegen::generate_wasm(&with_db, &mir_module).finish();

    let results = runtime::test::run(&core_bytes, None).expect("run tests");
    assert_eq!(results.len(), 1);
    match &results[0].outcome {
        runtime::test::Outcome::Fail(msg) => assert!(
            msg.contains("expected-failure-from-test-fixture"),
            "the raised message is reported verbatim, got: {msg}"
        ),
        other => panic!("expected a reported failure, got {other:?}"),
    }
}




/// The runtime runs `{test}` functions off the core module directly — no
/// component, no second engine, no WASI world. Pass, explicit failure and trap
/// must each be reported as themselves.
#[rstest]
fn runtime_runs_tests_from_the_core_module(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    add := a + b;
END_FUNCTION

FUNCTION assert_eq_int
VAR_INPUT value : INT; target : INT; END_VAR
    IF value <> target THEN
        __RAISE('');
    END_IF;
END_FUNCTION

{test}
FUNCTION test_that_passes
    assert_eq_int(value := add(a := 10, b := 32), target := 42);
END_FUNCTION

{test}
FUNCTION test_that_fails
    __RAISE('deliberate failure');
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core = wasm_codegen::generate_wasm(&with_db, &mir_module).finish();

    let found = runtime::test::discover(&core);
    assert_eq!(found.len(), 2, "both tests are in the core module's manifest");

    let results = runtime::test::run(&core, None).expect("run tests");
    assert_eq!(results.len(), 2);

    let passed = results
        .iter()
        .find(|r| r.entry.path.contains("test_that_passes"))
        .expect("the passing test ran");
    assert_eq!(passed.outcome, runtime::test::Outcome::Pass);

    let failed = results
        .iter()
        .find(|r| r.entry.path.contains("test_that_fails"))
        .expect("the failing test ran");
    match &failed.outcome {
        runtime::test::Outcome::Fail(msg) => {
            assert!(
                msg.contains("deliberate failure"),
                "the program's own message survives: {msg}"
            );
        }
        other => panic!("expected a reported failure, got {other:?}"),
    }

    // A filter selects a subset by path.
    let only = runtime::test::run(&core, Some("passes")).expect("filtered run");
    assert_eq!(only.len(), 1);
}

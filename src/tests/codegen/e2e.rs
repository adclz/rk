// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! End-to-end: compile IEC to a core module and run its `{test}` functions
//! through the runtime — the same loader a plant uses, no component wrapper.

use rstest::rstest;

use super::with_db;

/// Why a test failed, when the test itself reported it: a failed `ASSERT` or
/// a `__RAISE` comes back through the result area with the program's message.
/// A trap is a failure too, but one the test never got to report, and the
/// runner says so; a test here that expects a message does not accept one.
fn reported_failure(record: &debug_format::test_report::TestRecord) -> &str {
    let reason = record
        .reason
        .as_deref()
        .unwrap_or_else(|| panic!("expected a reported failure, got a pass: {record:?}"));
    assert!(
        !reason.starts_with("trapped"),
        "expected a reported failure, got a trap: {reason}"
    );
    reason
}

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

    let core_bytes = super::compile_to_wasm_as_built(&mut with_db, source);

    // The manifest is embedded in the module as a custom section — no sidecar.
    let results = crate::tests::codegen::run_tests(&core_bytes, None).expect("run tests");
    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|r| r.passed()),
        "Expected all e2e tests to pass"
    );
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

    let core_bytes = super::compile_to_wasm_as_built(&mut with_db, source);

    let results = crate::tests::codegen::run_tests(&core_bytes, None).expect("run tests");
    let failures = results.iter().filter(|r| !r.passed()).count();
    assert_eq!(
        failures, 1,
        "Expected exactly one failing test (`__RAISE` propagated as failure)"
    );
}

/// A test may have a result: it is a local of the test, which reads it back
/// whatever its type, and nothing else reads it.
#[rstest]
fn test_e2e_a_test_reads_its_own_result(mut with_db: db::RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT a : INT; b : REAL; END_STRUCT; END_TYPE
TYPE Row : ARRAY[0..3] OF INT; END_TYPE

{test}
FUNCTION t_real : REAL
    t_real := 1.5;
    IF t_real <= 1.0 THEN __RAISE('real'); END_IF;
END_FUNCTION

{test}
FUNCTION t_struct : Pair
    t_struct.a := 7;
    t_struct.b := 1.5;
    IF t_struct.a <> 7 OR t_struct.b <= 1.0 THEN __RAISE('struct'); END_IF;
END_FUNCTION

{test}
FUNCTION t_string : STRING
    t_string := 'abc';
    IF t_string <> 'abc' THEN __RAISE('string'); END_IF;
END_FUNCTION

{test}
FUNCTION t_array : Row
    t_array[1] := 7;
    IF t_array[1] <> 7 THEN __RAISE('array'); END_IF;
END_FUNCTION
    "#;

    let core_bytes = super::compile_to_wasm_as_built(&mut with_db, source);
    let results = crate::tests::codegen::run_tests(&core_bytes, None).expect("run tests");
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|r| r.passed()), "{results:?}");
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

    let core_bytes = super::compile_to_wasm_as_built(&mut with_db, source);

    let results = crate::tests::codegen::run_tests(&core_bytes, None).expect("run tests");
    assert_eq!(results.len(), 1);
    let reason = reported_failure(&results[0]);
    assert!(
        reason.contains("expected-failure-from-test-fixture"),
        "the raised message is reported verbatim, got: {reason}"
    );
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

    let core = super::compile_to_wasm_as_built(&mut with_db, source);

    let found = crate::tests::codegen::discover_tests(&core);
    assert_eq!(
        found.len(),
        2,
        "both tests are in the core module's manifest"
    );

    let results = crate::tests::codegen::run_tests(&core, None).expect("run tests");
    assert_eq!(results.len(), 2);

    let passed = results
        .iter()
        .find(|r| r.name.contains("test_that_passes"))
        .expect("the passing test ran");
    assert!(passed.passed(), "{passed:?}");

    let failed = results
        .iter()
        .find(|r| r.name.contains("test_that_fails"))
        .expect("the failing test ran");
    let reason = reported_failure(failed);
    assert!(
        reason.contains("deliberate failure"),
        "the program's own message survives: {reason}"
    );

    // A filter selects a subset by path.
    let only = crate::tests::codegen::run_tests(&core, Some("passes")).expect("filtered run");
    assert_eq!(only.len(), 1);
}

/// A STRING crosses to a host function intact.
///
/// This is the core ABI the component's canonical one replaced: a STRING
/// `VAR_INPUT` flattens to `(ptr, len)`, and the host reads those bytes out of
/// the linear memory it already owns. No adapter, no realloc, no lifting — the
/// host has the length in hand.
#[rstest]
fn a_string_reaches_a_host_import_intact(mut with_db: db::RootDatabase) {
    use std::sync::{Arc, Mutex};

    let source = r#"
{extern 'host' 'capture-msg'}
FUNCTION capture_msg
VAR_INPUT msg : STRING; END_VAR
END_FUNCTION

{export}
FUNCTION send : INT
    capture_msg(msg := 'hello from ST');
    send := 1;
END_FUNCTION
    "#;

    let wasm = super::compile_to_wasm_as_built(&mut with_db, source);

    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&captured);

    let _: i32 = super::execute_wasm_with_imports(&wasm, "send", (), move |linker| {
        linker
            .func_wrap(
                "host",
                "capture-msg",
                move |mut caller: wasmtime::Caller<'_, ()>, ptr: i32, len: i32| {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .expect("the module's linear memory");
                    let mut buf = vec![0u8; len as usize];
                    memory
                        .read(&caller, ptr as usize, &mut buf)
                        .expect("string bytes are inside linear memory");
                    *sink.lock().unwrap() = Some(String::from_utf8_lossy(&buf).into_owned());
                },
            )
            .expect("register host import");
    });

    assert_eq!(
        captured.lock().unwrap().as_deref(),
        Some("hello from ST"),
        "the host received the literal the ST source passed"
    );
}

/// A STRING returned to the host arrives intact, the other direction of the
/// same flattening: two i32 results, and the host reads the bytes itself.
#[rstest]
fn a_returned_string_reaches_the_host_intact(mut with_db: db::RootDatabase) {
    let source = r#"
{export}
FUNCTION greet : STRING
    greet := 'returned from ST';
END_FUNCTION
    "#;

    let wasm = super::compile_to_wasm_as_built(&mut with_db, source);

    let engine = super::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).expect("valid module");
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);

    let greet = instance
        .get_typed_func::<(), (i32, i32)>(&mut store, "greet")
        .expect("a STRING return is (ptr, len)");
    let (ptr, len) = greet.call(&mut store, ()).expect("call greet");

    let mut buf = vec![0u8; len as usize];
    memory
        .read(&store, ptr as usize, &mut buf)
        .expect("the returned bytes are inside linear memory");
    assert_eq!(
        String::from_utf8_lossy(&buf),
        "returned from ST",
        "the host reads the string the function built"
    );
}

/// A `RETURN` leaves a test as reaching its end does, wherever it stands and whatever
/// the test's result, through the optimizer too. It built an invalid module: the
/// wrapper returns the address of its result area, and a RETURN left nothing.
#[rstest]
fn test_e2e_a_return_ends_a_test(mut with_db: db::RootDatabase) {
    use crate::tests::utils::{skip_without_wasm_opt, wasm_opt_installed};

    let source = r#"
TYPE Pair : STRUCT a : INT; b : INT; END_STRUCT; END_TYPE

FUNCTION recurse : INT
VAR_INPUT n : INT; END_VAR
    IF n = 0 THEN
        recurse := 0;
        RETURN;
    END_IF;
    recurse := recurse(n - 1) + 1;
END_FUNCTION

{test} FUNCTION at_once
    RETURN;
END_FUNCTION

{test} FUNCTION before_a_raise
VAR i : INT; END_VAR
    IF i = 0 THEN
        RETURN;
    END_IF;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION with_an_int_result : INT
    with_an_int_result := 1;
    RETURN;
END_FUNCTION

{test} FUNCTION with_a_real_result : REAL
    with_a_real_result := 1.5;
    RETURN;
END_FUNCTION

{test} FUNCTION with_a_string_result : STRING
    with_a_string_result := 'text';
    RETURN;
END_FUNCTION

{test} FUNCTION with_a_struct_result : Pair
    with_a_struct_result.a := 1;
    RETURN;
END_FUNCTION

{test} FUNCTION in_a_try
    __TRY
        RETURN;
    __CATCH
        __RAISE('not reached');
    __END_TRY;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION in_a_catch
    __TRY
        __RAISE('caught');
    __CATCH
        RETURN;
    __END_TRY;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION through_a_finally
    __TRY
        RETURN;
    __FINALLY
        ;
    __END_TRY;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION in_a_loop
VAR i : INT; END_VAR
    FOR i := 0 TO 3 DO
        IF i = 2 THEN
            RETURN;
        END_IF;
    END_FOR;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION in_a_while_in_a_case
VAR i : INT; END_VAR
    CASE i OF
        0:
            WHILE TRUE DO
                RETURN;
            END_WHILE;
    END_CASE;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION after_a_recursive_call
    IF recurse(3) = 3 THEN
        RETURN;
    END_IF;
    __RAISE('not reached');
END_FUNCTION

{test} FUNCTION fails_before_its_return
    __RAISE('reached');
    RETURN;
END_FUNCTION

{test} FUNCTION fails_in_its_finally
    __TRY
        RETURN;
    __FINALLY
        __RAISE('from the finally');
    __END_TRY;
END_FUNCTION
"#;
    let assert_results = |wasm: &[u8], built: &str| {
        let results = crate::tests::codegen::run_tests(wasm, None).expect("run tests");
        assert_eq!(results.len(), 14, "{built}");
        let mut failed: Vec<_> = results
            .iter()
            .filter(|r| !r.passed())
            .map(|r| (r.name.as_str(), reported_failure(r)))
            .collect();
        failed.sort_unstable();
        assert_eq!(
            failed,
            [
                ("fails_before_its_return", "reached"),
                ("fails_in_its_finally", "from the finally"),
            ],
            "{built}"
        );
    };

    let wasm = super::compile_to_wasm_as_built(&mut with_db, source);
    assert_results(&wasm, "as built");

    if !wasm_opt_installed() {
        skip_without_wasm_opt("a RETURN in an optimized test");
        return;
    }
    for level in ["1", "3", "4", "s", "z"] {
        let optimized = rk::compiler::optimize_wasm_release(wasm.clone(), level, false)
            .unwrap_or_else(|e| panic!("-O{level}: {e}"));
        assert_results(&optimized, level);
    }
}

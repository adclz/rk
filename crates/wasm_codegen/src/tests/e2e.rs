//! End-to-end test: compile IEC → core WASM → component → run with wasmtime component runner.

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
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    // Write component + manifest to temp dir
    let tmp = std::env::temp_dir().join("rk_e2e_test");
    let build_dir = tmp.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).unwrap();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &component_bytes).unwrap();

    // The manifest is embedded in the component as a custom section — no sidecar.
    let failures = rk::test_runner::run_tests(&wasm_path, None);
    let _ = std::fs::remove_dir_all(&tmp);
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
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    let tmp = std::env::temp_dir().join("rk_e2e_failing_test_runner");
    let build_dir = tmp.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).unwrap();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &component_bytes).unwrap();

    // The manifest is embedded in the component as a custom section — no sidecar.
    let failures = rk::test_runner::run_tests(&wasm_path, None);
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(
        failures, 1,
        "Expected exactly one failing test (`__RAISE` propagated as failure)"
    );
}

/// Reproduce the "local index out of bounds" wasm-validation failure
/// triggered by `__RAISE(CONCAT("lit", msg))` — a STRING-returning
/// function call nested as the message expression of `__RAISE`.
/// Captures the malformed function index + offset for triage.
#[rstest]
fn test_e2e_raise_with_concat_arg_validates(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION str_concat : STRING
VAR_INPUT a : STRING; b : STRING; END_VAR
    {wasm 'str.concat' (params a b) (result str_concat)}
END_FUNCTION

FUNCTION push_str
VAR_INPUT a : STRING; END_VAR
VAR_IN_OUT b : STRING; END_VAR
    b := str_concat(b, a);
END_FUNCTION

FUNCTION any_to_string : STRING
VAR_INPUT v : ANY; END_VAR
    any_to_string := 'X';
END_FUNCTION

FUNCTION len_of : UDINT
VAR_INPUT s : STRING; END_VAR
    {wasm 'str.byte_len' (params s) (result len_of)}
END_FUNCTION

FUNCTION ASSERT
VAR_INPUT
    value : BOOL;
    message : STRING := '';
END_VAR
    IF NOT value THEN
        IF len_of(message) = 0 THEN
            message := 'assertion failed: ';
            // user's actual call: positional arg 2 (=`b`, VAR_IN_OUT)
            // receives `any_to_string(value)` — a function-call result,
            // not an lvalue. Likely the actual bug trigger.
            push_str(message, any_to_string(value));
            __RAISE(message);
        ELSE
            __RAISE(str_concat('assertion failed: ', message));
        END_IF;
    END_IF;
END_FUNCTION

{test}
FUNCTION test_uses_assert
    ASSERT(FALSE, '');
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let mut features = wasmparser::WasmFeatures::default();
    features.insert(wasmparser::WasmFeatures::EXCEPTIONS);
    let result = wasmparser::Validator::new_with_features(features).validate_all(&core_bytes);
    match result {
        Ok(_) => {}
        Err(e) => {
            std::fs::write("/tmp/concat_repro.wasm", &core_bytes).ok();
            panic!(
                "validation failed: {}\nWrote core wasm to /tmp/concat_repro.wasm for inspection",
                e
            );
        }
    }
}

/// Probe the Rust-panic → `$rk_exception` propagation end-to-end from
/// IEC code. The {wasm} pragma calls `rk.div_i32_checked` (a bare Rust
/// `a / b`); when the divisor is 0 Rust's compiler-emitted divide-by-zero
/// check panics with `"attempt to divide by zero"`, the panic handler
/// re-throws it as `$rk_exception`, and the test wrapper catches it.
/// If everything is wired correctly, the test runner sees Err with the
/// Rust panic message; if anything in the chain is misrouted, the call
/// silently succeeds (test passes) or hard-traps (test fails with a
/// wasm trap rather than the typed message).
#[rstest]
fn test_e2e_rust_panic_propagates_from_wasm_pragma(mut with_db: db::RootDatabase) {
    use wasmtime::component::{Component, Linker, Val};
    use wasmtime::{Config, Engine, Store};

    let source = r#"
FUNCTION checked_div : DINT
VAR_INPUT
    a : DINT;
    b : DINT;
END_VAR
VAR
    result : DINT;
END_VAR
    {wasm 'rk.div_i32_checked' (params a b) (result result)}
    checked_div := result;
END_FUNCTION

{test}
FUNCTION test_should_panic_on_div_zero
VAR x : DINT; END_VAR
    x := checked_div(a := 10, b := 0);
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    let engine = {
        let mut c = Config::new();
        c.wasm_exceptions(true);
        Engine::new(&c).expect("engine")
    };
    let component = Component::new(&engine, &component_bytes).expect("valid component");
    let linker: Linker<()> = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("instantiate");

    let func = instance
        .get_func(&mut store, "test-should-panic-on-div-zero")
        .expect("export");
    let mut results = [Val::Bool(false)];
    func.call(&mut store, &[], &mut results)
        .expect("call must complete (trap → uncatchable would Err here)");

    match &results[0] {
        Val::Result(Err(Some(payload))) => match payload.as_ref() {
            Val::String(s) => assert_eq!(
                s, "attempt to divide by zero",
                "Err payload must be Rust's verbatim panic message"
            ),
            other => panic!("Err payload should be String, got {:?}", other),
        },
        Val::Result(Ok(_)) => {
            panic!("test unexpectedly passed — the panic→exception chain is broken somewhere");
        }
        other => panic!("expected Result::Err, got {:?}", other),
    }
}

/// Verify the test runner extracts the assertion message text from the
/// `result<unit, string>` Err payload — not just that a failure happened.
/// The fixture uses `__RAISE 'specific-marker-text'` so a successful
/// extraction shows up as a `Result::Err(Some(Val::String("specific-marker-text")))`
/// in the typed component return.
#[rstest]
fn test_e2e_failing_test_surfaces_raise_message(mut with_db: db::RootDatabase) {
    use wasmtime::component::{Component, Linker, Val};
    use wasmtime::{Config, Engine, Store};

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
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    let engine = {
        let mut c = Config::new();
        c.wasm_exceptions(true);
        Engine::new(&c).expect("engine")
    };
    let component = Component::new(&engine, &component_bytes).expect("valid component");
    let linker: Linker<()> = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("instantiate");

    let func = instance
        .get_func(&mut store, "test-fails-with-message")
        .expect("export");
    let mut results = [Val::Bool(false)];
    func.call(&mut store, &[], &mut results).expect("call");

    match &results[0] {
        Val::Result(Err(Some(payload))) => match payload.as_ref() {
            Val::String(s) => assert_eq!(
                s, "expected-failure-from-test-fixture",
                "lifted Err payload should match raised STRING"
            ),
            other => panic!("Err payload should be String, got {:?}", other),
        },
        other => panic!("expected Result::Err, got {:?}", other),
    }
}

/// Verify that a non-empty STRING literal passed from guest to host survives
/// the canonical ABI `lower` adapter intact.
///
/// Guest calls an imported host function `capture-msg(msg: string)` with a
/// string literal. Host-side closure records the received String. Assert it
/// matches what the guest sent.
#[rstest]
fn test_string_guest_to_host(mut with_db: db::RootDatabase) {
    use std::sync::{Arc, Mutex};
    use wasmtime::component::{Component, Linker};
    use wasmtime::{Engine, Store};

    let source = r#"
FUNCTION capture_msg
VAR_INPUT msg : STRING; END_VAR
    {extern 'host' 'capture-msg' (params msg)}
END_FUNCTION

{test}
FUNCTION test_send_string
    capture_msg(msg := 'hello from ST');
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    // `{test}` functions are codegen-wrapped in a `try_table`, so the
    // exceptions proposal must be enabled to load the component.
    let engine = {
        let mut c = wasmtime::Config::new();
        c.wasm_exceptions(true);
        Engine::new(&c).expect("engine")
    };
    let component = Component::new(&engine, &component_bytes).expect("valid component");

    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let captured_clone = Arc::clone(&captured);

    let mut linker: Linker<()> = Linker::new(&engine);
    linker
        .root()
        .func_wrap(
            "host-capture-msg",
            move |_ctx: wasmtime::StoreContextMut<'_, ()>,
                  (msg,): (String,)|
                  -> wasmtime::Result<()> {
                *captured_clone.lock().unwrap() = Some(msg);
                Ok(())
            },
        )
        .expect("register host import");

    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("instantiate");

    let func = instance
        .get_func(&mut store, "test-send-string")
        .expect("export");
    // `{test}` functions return `result<unit, string>` at the component
    // boundary — the placeholder gets filled by `call`.
    let mut results = [wasmtime::component::Val::Bool(false)];
    func.call(&mut store, &[], &mut results).expect("call test");
    match &results[0] {
        wasmtime::component::Val::Result(Ok(_)) => {}
        wasmtime::component::Val::Result(Err(payload)) => {
            panic!("test failed with assertion: {:?}", payload.as_deref())
        }
        other => panic!("unexpected result shape: {:?}", other),
    }

    let received = captured.lock().unwrap().clone();
    assert_eq!(received.as_deref(), Some("hello from ST"));
}

/// Verify that a STRING *returned* from a guest function reaches the host
/// as a proper component-level `string`.
///
/// Exercises: multi-value core return `(i32, i32)`, canon `lift_func` with
/// `[Memory(0), UTF8]`, host-side `String` reception.
#[rstest]
fn test_string_guest_return(mut with_db: db::RootDatabase) {
    use wasmtime::component::{Component, Linker};
    use wasmtime::{Engine, Store};

    let source = r#"
FUNCTION greet : STRING
    greet := 'hello from ST';
END_FUNCTION

FUNCTION check_greet
VAR_INPUT msg : STRING; END_VAR
    {extern 'host' 'check-greet' (params msg)}
END_FUNCTION

{test}
FUNCTION test_greet_returns_string
    check_greet(msg := greet());
END_FUNCTION
    "#;

    // The test function calls an imported `check-greet(s: string)` with the
    // result of `greet()`. The host-side closure asserts the received bytes.
    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    // `{test}` functions are codegen-wrapped in a `try_table`, so the
    // exceptions proposal must be enabled to load the component.
    let engine = {
        let mut c = wasmtime::Config::new();
        c.wasm_exceptions(true);
        Engine::new(&c).expect("engine")
    };
    let component = Component::new(&engine, &component_bytes).expect("valid component");

    use std::sync::{Arc, Mutex};
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let captured_clone = Arc::clone(&captured);

    let mut linker: Linker<()> = Linker::new(&engine);
    linker
        .root()
        .func_wrap(
            "host-check-greet",
            move |_ctx: wasmtime::StoreContextMut<'_, ()>,
                  (msg,): (String,)|
                  -> wasmtime::Result<()> {
                *captured_clone.lock().unwrap() = Some(msg);
                Ok(())
            },
        )
        .expect("register host import");

    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("instantiate");

    let func = instance
        .get_func(&mut store, "test-greet-returns-string")
        .expect("export");
    // `{test}` functions return `result<unit, string>` at the component
    // boundary — the placeholder gets filled by `call`.
    let mut results = [wasmtime::component::Val::Bool(false)];
    func.call(&mut store, &[], &mut results).expect("call test");
    match &results[0] {
        wasmtime::component::Val::Result(Ok(_)) => {}
        wasmtime::component::Val::Result(Err(payload)) => {
            panic!("test failed with assertion: {:?}", payload.as_deref())
        }
        other => panic!("unexpected result shape: {:?}", other),
    }

    assert_eq!(
        captured.lock().unwrap().as_deref(),
        Some("hello from ST"),
        "host should receive the string returned by greet()"
    );
}

#[rstest]
fn test_component_wrapping(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT a : INT; b : INT; END_VAR
    add := a + b;
END_FUNCTION

FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs.INT' (params x) (result my_abs)}
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module =
        mir::lower::lower_module::lower_module(&with_db, sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();

    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    assert!(component_bytes.len() > 8, "Component should have content");
    assert_eq!(
        &component_bytes[0..4],
        b"\0asm",
        "Should start with WASM magic"
    );
}

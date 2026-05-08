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

FUNCTION __ASSERT_FAIL
VAR_INPUT message : STRING; END_VAR
    {extern 'assert' 'fail' (params message)}
END_FUNCTION

FUNCTION assert_eq_int
VAR_INPUT value : INT; target : INT; END_VAR
    IF value <> target THEN
        __ASSERT_FAIL(message := '');
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
        mir::lower::lower_module::lower_module(&with_db, &sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    // Write component + manifest to temp dir
    let tmp = std::env::temp_dir().join("rk_e2e_test");
    let build_dir = tmp.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).unwrap();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &component_bytes).unwrap();
    std::fs::write(
        build_dir.join("manifest"),
        mir_module.test_manifest.to_msgpack(),
    )
    .unwrap();

    let failures = rk::test_runner::run_tests(&wasm_path, &tmp, None);
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(failures, 0, "Expected all e2e tests to pass");
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
        mir::lower::lower_module::lower_module(&with_db, &sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    let engine = Engine::default();
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
    func.call(&mut store, &[], &mut []).expect("call test");

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
        mir::lower::lower_module::lower_module(&with_db, &sem_idx).expect("MIR lowering failed");
    let core_bytes = crate::generate_wasm(&with_db, &mir_module).finish();
    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    let engine = Engine::default();
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
    func.call(&mut store, &[], &mut []).expect("call test");

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
        mir::lower::lower_module::lower_module(&with_db, &sem_idx).expect("MIR lowering failed");
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

//! End-to-end test: compile IEC → WASM → run with wasmtime.

use rstest::rstest;

use super::{compile_to_wasm, with_db};

#[rstest]
fn test_e2e_runtime(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs.INT' (params x) (result my_abs)}
END_FUNCTION

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

{test}
FUNCTION test_abs
    assert_eq_int(value := my_abs(x := -7), target := 7);
END_FUNCTION
    "#;

    let file = super::add_source(&mut with_db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module = mir::lower::lower_module::lower_module(&with_db, &sem_idx)
        .expect("MIR lowering failed");
    let wasm_bytes = crate::from_mir::generate_wasm(&with_db, &mir_module).finish();

    // Write wasm + manifest to temp dir
    let tmp = std::env::temp_dir().join("rk_e2e_test");
    let build_dir = tmp.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).unwrap();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &wasm_bytes).unwrap();
    std::fs::write(build_dir.join("manifest"), mir_module.test_manifest.to_msgpack()).unwrap();

    let failures = rk::test_runner::run_tests(&wasm_path, &tmp, None);
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(failures, 0, "Expected all e2e tests to pass");
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
    let mir_module = mir::lower::lower_module::lower_module(&with_db, &sem_idx)
        .expect("MIR lowering failed");
    let core_bytes = crate::from_mir::generate_wasm(&with_db, &mir_module).finish();

    let component_bytes = crate::component::wrap_in_component(&with_db, &core_bytes, &mir_module)
        .expect("Component wrapping failed");

    // Verify it's a valid component (starts with component magic)
    assert!(component_bytes.len() > 8, "Component should have content");
    assert_eq!(&component_bytes[0..4], b"\0asm", "Should start with WASM magic");
    // Component version is different from core module version
    assert_ne!(&component_bytes[4..8], &[1, 0, 0, 0], "Should NOT be core module version");
}

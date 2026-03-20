//! End-to-end test: compile IEC → WASM → run in Python with wasmtime.

use rstest::rstest;
use std::io::Write;

use super::{compile_to_wasm, with_db};

#[rstest]
fn test_e2e_python_runtime(mut with_db: db::RootDatabase) {
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
    {extern 'assert' 'fail'}
END_FUNCTION

FUNCTION assert_eq_int
VAR_INPUT value : INT; target : INT; END_VAR
    IF value <> target THEN
        __ASSERT_FAIL();
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

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Write WASM to a temp file
    let wasm_path = std::env::temp_dir().join("rk_e2e_test.wasm");
    let mut file = std::fs::File::create(&wasm_path).unwrap();
    file.write_all(&wasm_bytes).unwrap();

    // Find the Python test runner
    let runner_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("runtimes")
        .join("python")
        .join("test_runner.py");

    assert!(
        runner_path.exists(),
        "Python test runner not found at: {}",
        runner_path.display()
    );

    // Run with Python
    let output = std::process::Command::new("python3")
        .arg(&runner_path)
        .arg(&wasm_path)
        .output()
        .expect("Failed to run python3 — make sure Python 3 and wasmtime are installed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Cleanup
    let _ = std::fs::remove_file(&wasm_path);

    if !output.status.success() {
        panic!(
            "Python e2e test failed!\nstdout: {}\nstderr: {}",
            stdout, stderr
        );
    }

    assert!(
        stdout.contains("3 tests run: 3 passed, 0 failed"),
        "Expected all tests to pass, got:\n{}",
        stdout
    );
}

//! FUNCTION overloads lower to distinct WASM symbols (via the arity
//! discriminant) and each call routes to the right one.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

// Two `add` overloads (1-arg adds one, 2-arg sums) coexist as distinct wasm
// functions; `test` calls both and gets each result.
#[rstest]
fn overloaded_functions_execute_independently(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION

        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test : INT
            test := add(10) + add(3, 4);
        END_FUNCTION
    "#;

    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 18, "add/1(10) = 11, add/2(3, 4) = 7");
}

// Overloading works for non-integer types too — two REAL overloads, one per
// arity, each lowered and called independently.
#[rstest]
fn overloaded_real_functions(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION avg : REAL
        VAR_INPUT a : REAL; END_VAR
            avg := a;
        END_FUNCTION

        FUNCTION avg : REAL
        VAR_INPUT a : REAL; b : REAL; END_VAR
            avg := (a + b) / 2.0;
        END_FUNCTION

        FUNCTION test : REAL
            test := avg(3.0) + avg(2.0, 8.0);   // 3.0 + 5.0
        END_FUNCTION
    "#;

    let wasm = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 8.0, "avg/1(3.0) = 3.0, avg/2(2.0, 8.0) = 5.0");
}

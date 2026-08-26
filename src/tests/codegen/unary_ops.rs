//! Unary operator codegen: negation and NOT across type widths.
//! (Regression: 64-bit integer negation emitted `value; i64.const 0; i64.sub`
//! = `value - 0` — a silent no-op.)

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// The bug: `-x` on a 64-bit integer must actually negate.
#[rstest]
fn neg_lint(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LINT
        VAR x : LINT := 5; END_VAR
            test := -x;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, -5, "LINT negation: -(5)");
}

/// 64-bit negation of a computed expression.
#[rstest]
fn neg_lint_expr(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LINT
        VAR a : LINT := 40; b : LINT := 2; END_VAR
            test := -(a + b);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, -42, "-(40 + 2)");
}

/// 32-bit negation (already used the two's-complement trick).
#[rstest]
fn neg_dint(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : DINT
        VAR b : DINT := 100000; END_VAR
            test := -b;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, -100000, "-(100000)");
}

/// Float negation, both widths.
#[rstest]
fn neg_real(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
        VAR r : REAL := 2.5; END_VAR
            test := -r;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, -2.5, "-(2.5)");
}

/// 64-bit float negation.
#[rstest]
fn neg_lreal(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LREAL
        VAR l : LREAL := 10.25; END_VAR
            test := -l;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: f64 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, -10.25, "-(10.25)");
}

/// Double negation round-trips.
#[rstest]
fn neg_double(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LINT
        VAR x : LINT := 9; END_VAR
            test := -(-x);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 9, "-(-9)");
}

/// NOT on BOOL.
#[rstest]
fn not_bool(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR b : BOOL := FALSE; END_VAR
            IF NOT b THEN
                test := 1;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "NOT FALSE = TRUE");
}

/// Bitwise NOT on 64-bit LWORD.
#[rstest]
fn not_lword(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LWORD
        VAR w : LWORD := 16#F0F0; END_VAR
            test := NOT w;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, !0xF0F0u64 as i64, "bitwise complement");
}

/// Bitwise NOT on a BYTE compared against its 8-bit complement: probes
/// whether the high bits left by `xor -1` break narrow-type comparison.
#[rstest]
fn not_byte_compare(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR b : BYTE := 16#0F; END_VAR
            IF NOT b = 16#F0 THEN
                test := 1;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "NOT 16#0F = 16#F0 for BYTE");
}

/// The `AND NOT` composition on WORDs (IL's ANDN: `result AND NOT operand`,
/// bitwise one's complement — IEC 7.2.3.3). Requires NOT to type as WORD
/// (ANY_BIT) and emit a width-masked complement.
#[rstest]
fn word_and_not(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : WORD
        VAR a : WORD := 16#00FF; b : WORD := 16#0F0F; END_VAR
            test := a AND NOT b;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 0x00F0, "16#00FF AND NOT 16#0F0F = 16#00F0");
}

/// Bitwise AND/OR/XOR on 64-bit LWORDs (the i64 emit path).
#[rstest]
fn lword_bitwise_ops(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LWORD
        VAR a : LWORD := 16#FF00FF00FF00FF00; b : LWORD := 16#0FF00FF00FF00FF0; END_VAR
            test := (a AND b) OR (a XOR b);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "test", ());
    let a = 0xFF00FF00FF00FF00u64;
    let b = 0x0FF00FF00FF00FF0u64;
    assert_eq!(result as u64, (a & b) | (a ^ b), "(a AND b) OR (a XOR b)");
}

/// `OR NOT` and `XOR NOT` compositions on BYTE stay within the 8-bit width.
#[rstest]
fn byte_or_xor_not(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR a : BYTE := 16#0F; b : BYTE := 16#33; END_VAR
            IF (a OR NOT b) = 16#CF AND (a XOR NOT b) = 16#C3 THEN
                test := 1;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "OR NOT / XOR NOT byte compositions");
}

//! Bit string operations tests (BYTE, WORD, DWORD, LWORD).

use crate::tests::{compile_to_wasm, execute_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_lword_passthrough(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_pass : LWORD
        VAR_INPUT
            a : LWORD;
            b : LWORD;
        END_VAR
            test_pass := a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result: i64 = execute_wasm(&wasm_bytes, "test_pass", (42i64, 100i64));
    assert_eq!(result, 42, "Should return first parameter");
}

#[rstest]
fn test_byte_and(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_byte_and : BYTE
        VAR_INPUT
            a : BYTE;
            b : BYTE;
        END_VAR
            test_byte_and := a AND b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: 0xFF AND 0x0F = 0x0F
    let result: i32 = execute_wasm(&wasm_bytes, "test_byte_and", (0xFFi32, 0x0Fi32));
    assert_eq!(result, 0x0F, "0xFF AND 0x0F should be 0x0F");
}

#[rstest]
fn test_word_or(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_word_or : WORD
        VAR_INPUT
            a : WORD;
            b : WORD;
        END_VAR
            test_word_or := a OR b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: 0xF0F0 OR 0x0F0F = 0xFFFF
    let result: i32 = execute_wasm(&wasm_bytes, "test_word_or", (0xF0F0i32, 0x0F0Fi32));
    assert_eq!(result, 0xFFFF, "0xF0F0 OR 0x0F0F should be 0xFFFF");
}

#[rstest]
fn test_dword_xor(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_dword_xor : DWORD
        VAR_INPUT
            a : DWORD;
            b : DWORD;
        END_VAR
            test_dword_xor := a XOR b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: 0xFFFF0000 XOR 0x0000FFFF = 0xFFFFFFFF
    let result: i32 = execute_wasm(&wasm_bytes, "test_dword_xor", (0xFFFF0000u32 as i32, 0x0000FFFFi32));
    assert_eq!(result, -1, "0xFFFF0000 XOR 0x0000FFFF should be 0xFFFFFFFF");
}

#[rstest]
fn test_lword_and(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_lword_and : LWORD
        VAR_INPUT
            a : LWORD;
            b : LWORD;
        END_VAR
            test_lword_and := a AND b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: 0xFFFFFFFFFFFFFFFF AND 0x00000000FFFFFFFF = 0x00000000FFFFFFFF
    let result: i64 = execute_wasm(&wasm_bytes, "test_lword_and", (-1i64, 0x00000000FFFFFFFFi64));
    assert_eq!(result, 0x00000000FFFFFFFF, "LWORD AND operation");
}

#[rstest]
fn test_lword_or(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_lword_or : LWORD
        VAR_INPUT
            a : LWORD;
            b : LWORD;
        END_VAR
            test_lword_or := a OR b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: 0xFFFFFFFF00000000 OR 0x00000000FFFFFFFF = 0xFFFFFFFFFFFFFFFF
    let result: i64 = execute_wasm(&wasm_bytes, "test_lword_or", (0xFFFFFFFF00000000u64 as i64, 0x00000000FFFFFFFFi64));
    assert_eq!(result, -1, "LWORD OR operation should give all 1s");
}

#[rstest]
fn test_lword_xor(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_lword_xor : LWORD
        VAR_INPUT
            a : LWORD;
            b : LWORD;
        END_VAR
            test_lword_xor := a XOR b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: 0xAAAAAAAAAAAAAAAA XOR 0x5555555555555555 = 0xFFFFFFFFFFFFFFFF
    let result: i64 = execute_wasm(&wasm_bytes, "test_lword_xor", (0xAAAAAAAAAAAAAAAAu64 as i64, 0x5555555555555555i64));
    assert_eq!(result, -1, "Alternating bits XOR should give all 1s");
}

#[rstest]
fn test_byte_not(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_byte_not : BYTE
        VAR_INPUT
            a : BYTE;
        END_VAR
            test_byte_not := NOT a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: NOT 0x00 = 0xFF (in BYTE context, but WASM works with i32)
    let result: i32 = execute_wasm(&wasm_bytes, "test_byte_not", 0x00i32);
    // NOT flips all 32 bits, so 0x00000000 becomes 0xFFFFFFFF
    assert_eq!(result, -1, "NOT 0 should flip all bits to 1");

    // Test: NOT 0xFF = 0xFFFFFF00 (flips all bits)
    let result: i32 = execute_wasm(&wasm_bytes, "test_byte_not", 0xFFi32);
    assert_eq!(result, 0xFFFFFF00u32 as i32, "NOT 0xFF should flip bits");
}

#[rstest]
fn test_word_not(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_word_not : WORD
        VAR_INPUT
            a : WORD;
        END_VAR
            test_word_not := NOT a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: NOT 0x0000 = 0xFFFFFFFF
    let result: i32 = execute_wasm(&wasm_bytes, "test_word_not", 0x0000i32);
    assert_eq!(result, -1, "NOT 0 should flip all bits");

    // Test: NOT 0xFFFF = 0xFFFF0000
    let result: i32 = execute_wasm(&wasm_bytes, "test_word_not", 0xFFFFi32);
    assert_eq!(result, 0xFFFF0000u32 as i32, "NOT 0xFFFF");
}

#[rstest]
fn test_dword_not(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_dword_not : DWORD
        VAR_INPUT
            a : DWORD;
        END_VAR
            test_dword_not := NOT a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: NOT 0x00000000 = 0xFFFFFFFF
    let result: i32 = execute_wasm(&wasm_bytes, "test_dword_not", 0i32);
    assert_eq!(result, -1, "NOT 0 should be all 1s");

    // Test: NOT 0xAAAAAAAA = 0x55555555
    let result: i32 = execute_wasm(&wasm_bytes, "test_dword_not", 0xAAAAAAAAu32 as i32);
    assert_eq!(result, 0x55555555, "NOT 0xAAAAAAAA = 0x55555555");
}

#[rstest]
fn test_lword_not(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_lword_not : LWORD
        VAR_INPUT
            a : LWORD;
        END_VAR
            test_lword_not := NOT a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: NOT 0 = all 1s
    let result: i64 = execute_wasm(&wasm_bytes, "test_lword_not", 0i64);
    assert_eq!(result, -1, "NOT 0 should be all 1s");

    // Test: NOT 0xAAAAAAAAAAAAAAAA = 0x5555555555555555
    let result: i64 = execute_wasm(&wasm_bytes, "test_lword_not", 0xAAAAAAAAAAAAAAAAu64 as i64);
    assert_eq!(result, 0x5555555555555555, "NOT alternating bits");
}

#[rstest]
fn test_bool_not_still_works(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_bool_not : BOOL
        VAR_INPUT
            a : BOOL;
        END_VAR
            test_bool_not := NOT a;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: NOT TRUE = FALSE
    let result: i32 = execute_wasm(&wasm_bytes, "test_bool_not", 1i32);
    assert_eq!(result, 0, "NOT TRUE should be FALSE");

    // Test: NOT FALSE = TRUE
    let result: i32 = execute_wasm(&wasm_bytes, "test_bool_not", 0i32);
    assert_eq!(result, 1, "NOT FALSE should be TRUE");
}

#[rstest]
fn test_complex_bit_expression(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_complex : DWORD
        VAR_INPUT
            a : DWORD;
            b : DWORD;
            c : DWORD;
        END_VAR
            test_complex := (a AND b) OR (NOT c);
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Test: (0xFFFF0000 AND 0xFF00FF00) OR (NOT 0x00000000)
    //     = (0xFF000000) OR 0xFFFFFFFF
    //     = 0xFFFFFFFF
    let result: i32 = execute_wasm(
        &wasm_bytes,
        "test_complex",
        (0xFFFF0000u32 as i32, 0xFF00FF00u32 as i32, 0i32),
    );
    assert_eq!(result, -1, "Complex bit expression");
}

#[rstest]
fn test_bool_and_bit_mixing(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_mixing : BOOL
        VAR_INPUT
            a : BOOL;
            b : BOOL;
        END_VAR
        VAR
            bits : BYTE;
        END_VAR
            bits := BYTE#16#FF;
            test_mixing := a AND b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Boolean AND should work independently of bit operations
    let result: i32 = execute_wasm(&wasm_bytes, "test_mixing", (1i32, 1i32));
    assert_eq!(result, 1, "TRUE AND TRUE = TRUE");

    let result: i32 = execute_wasm(&wasm_bytes, "test_mixing", (1i32, 0i32));
    assert_eq!(result, 0, "TRUE AND FALSE = FALSE");
}

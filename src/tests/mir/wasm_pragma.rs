use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

#[rstest]
fn wasm_pragma_generates_intrinsic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION REAL_TO_INT : INT
VAR_INPUT IN : REAL; END_VAR
    {wasm 'i32.trunc_f32_s' (params IN) (result REAL_TO_INT)}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export REAL_TO_INT(Real) -> Int");
}

#[rstest]
fn wasm_pragma_any_monomorphizes_with_type_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SHL : ANY_BIT
VAR_INPUT
    IN : INTO(SHL);
    N : INT;
END_VAR
    {wasm IN 'shl' (params IN N) (result SHL)}
END_FUNCTION

FUNCTION test
VAR x : BYTE; w : WORD; END_VAR
    x := SHL(IN := BYTE#1, N := 4);
    w := SHL(IN := WORD#1, N := 8);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SHL.BYTE(Byte, Int) -> Byte
    export SHL.WORD(Word, Int) -> Word
    export test()
    ");
}

#[rstest]
fn wasm_pragma_explicit_instruction(mut with_db: RootDatabase) {
    // No type_ref — instruction used as-is
    let source = r#"
FUNCTION INT_TO_REAL : REAL
VAR_INPUT IN : INT; END_VAR
    {wasm 'f32.convert_i32_s' (params IN) (result INT_TO_REAL)}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export INT_TO_REAL(Int) -> Real");
}

#[rstest]
fn wasm_pragma_multiple_type_variants(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ROL : ANY_BIT
VAR_INPUT IN : INTO(ROL); N : INT; END_VAR
    {wasm IN 'rotl' (params IN N) (result ROL)}
END_FUNCTION

FUNCTION test
VAR b : BYTE; w : WORD; d : DWORD; END_VAR
    b := ROL(IN := BYTE#1, N := 1);
    w := ROL(IN := WORD#1, N := 1);
    d := ROL(IN := DWORD#1, N := 1);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export ROL.BYTE(Byte, Int) -> Byte
    export ROL.DWORD(DWord, Int) -> DWord
    export ROL.WORD(Word, Int) -> Word
    export test()
    ");
}

#[rstest]
fn wasm_pragma_no_result(mut with_db: RootDatabase) {
    // WASM instruction with no result (side-effect only)
    let source = r#"
FUNCTION nop
    {wasm 'nop'}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export nop()");
}

#[rstest]
fn wasm_conversion_all_directions(mut with_db: RootDatabase) {
    // Verify all conversion directions generate correct exports
    let source = r#"
FUNCTION INT_TO_REAL : REAL
VAR_INPUT IN : INT; END_VAR
    {wasm 'f32.convert_i32_s' (params IN) (result INT_TO_REAL)}
END_FUNCTION

FUNCTION REAL_TO_INT : INT
VAR_INPUT IN : REAL; END_VAR
    {wasm 'i32.trunc_sat_f32_s' (params IN) (result REAL_TO_INT)}
END_FUNCTION

FUNCTION INT_TO_LREAL : LREAL
VAR_INPUT IN : INT; END_VAR
    {wasm 'f64.convert_i32_s' (params IN) (result INT_TO_LREAL)}
END_FUNCTION

FUNCTION LINT_TO_INT : INT
VAR_INPUT IN : LINT; END_VAR
    {wasm 'i32.wrap_i64' (params IN) (result LINT_TO_INT)}
END_FUNCTION

FUNCTION test
VAR i : INT; r : REAL; lr : LREAL; li : LINT; END_VAR
    r := INT_TO_REAL(IN := 42);
    i := REAL_TO_INT(IN := 3.14);
    lr := INT_TO_LREAL(IN := 42);
    i := LINT_TO_INT(IN := LINT#100);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export INT_TO_LREAL(Int) -> LReal
    export INT_TO_REAL(Int) -> Real
    export LINT_TO_INT(LInt) -> Int
    export REAL_TO_INT(Real) -> Int
    export test()
    ");
}

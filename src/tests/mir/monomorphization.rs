use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

#[rstest]
fn local_any_function_monomorphizes(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SEL : ANY
VAR_INPUT
    G : BOOL;
    IN0 : INTO(SEL);
    IN1 : INTO(SEL);
END_VAR
    IF G THEN SEL := IN1; ELSE SEL := IN0; END_IF;
END_FUNCTION

FUNCTION test : INT
    test := SEL(G := TRUE, IN0 := 10, IN1 := 42);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SEL.INT(Bool, Int, Int) -> Int
    export test() -> Int
    ");
}

#[rstest]
fn chained_any_calls_assert_eq_with_add(mut with_db: RootDatabase) {
    // ASSERT_EQ accepts ANY, ADD returns ANY_NUM.
    // When called as ASSERT_EQ(value := ADD(1, 2), target := 3),
    // both should monomorphize to INT variants.
    let source = r#"
FUNCTION __ASSERT_FAIL
    {extern 'assert' 'fail'}
END_FUNCTION

FUNCTION ASSERT_EQ
VAR_INPUT
    value : ANY;
    target : INTO(value);
END_VAR
    IF value <> target THEN
        __ASSERT_FAIL();
    END_IF;
END_FUNCTION

FUNCTION ADD : ANY_MAGNITUDE
VAR_INPUT args : INTO(ADD)... END_VAR
END_FUNCTION

FUNCTION test
    ASSERT_EQ(value := ADD(1, 2), target := 3);
    ASSERT_EQ(value := ADD(1.0, 2.0), target := 3.0);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export ASSERT_EQ.INT(Int, Int)
    export ASSERT_EQ.REAL(Real, Real)
    export test()
    import assert.fail()
    ");
}

#[rstest]
fn multiple_concrete_types_from_different_call_sites(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION LIMIT : ANY_ELEMENTARY
VAR_INPUT
    MN : INTO(LIMIT);
    IN : INTO(LIMIT);
    MX : INTO(LIMIT);
END_VAR
    IF IN < MN THEN LIMIT := MN;
    ELSIF IN > MX THEN LIMIT := MX;
    ELSE LIMIT := IN;
    END_IF;
END_FUNCTION

FUNCTION test
VAR x : INT; y : REAL; END_VAR
    x := LIMIT(MN := 0, IN := 5, MX := 10);
    y := LIMIT(MN := 0.0, IN := 5.5, MX := 10.0);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export LIMIT.INT(Int, Int, Int) -> Int
    export LIMIT.REAL(Real, Real, Real) -> Real
    export test()
    ");
}

#[rstest]
fn extern_any_generates_typed_imports(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ABS : ANY_NUM
VAR_INPUT IN : INTO(ABS); END_VAR
    {extern 'math' 'abs' (params IN) (result ABS)}
END_FUNCTION

FUNCTION test
VAR a : INT; b : REAL; c : LINT; END_VAR
    a := ABS(IN := -1);
    b := ABS(IN := -1.5);
    c := ABS(IN := LINT#-100);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test()
    import math.abs.INT(Int) -> Int [from ABS]
    import math.abs.LINT(LInt) -> LInt [from ABS]
    import math.abs.REAL(Real) -> Real [from ABS]
    ");
}

#[rstest]
fn wasm_any_bit_generates_typed_intrinsics(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SHR : ANY_BIT
VAR_INPUT IN : INTO(SHR); N : INT; END_VAR
    {wasm IN 'shr_u' (params IN N) (result SHR)}
END_FUNCTION

FUNCTION test
VAR b : BYTE; d : DWORD; END_VAR
    b := SHR(IN := BYTE#16#FF, N := 4);
    d := SHR(IN := DWORD#16#FFFF, N := 8);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SHR.BYTE(Byte, Int) -> Byte
    export SHR.DWORD(DWord, Int) -> DWord
    export test()
    ");
}

#[rstest]
fn unused_any_function_not_exported(mut with_db: RootDatabase) {
    // An ANY function with no call sites should not generate monomorphized copies
    let source = r#"
FUNCTION IDENTITY : ANY
VAR_INPUT IN : INTO(IDENTITY); END_VAR
    IDENTITY := IN;
END_FUNCTION

FUNCTION test : INT
    test := 42;
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export IDENTITY.BOOL(Bool) -> Bool
    export IDENTITY.BYTE(Byte) -> Byte
    export IDENTITY.DINT(DInt) -> DInt
    export IDENTITY.DWORD(DWord) -> DWord
    export IDENTITY.INT(Int) -> Int
    export IDENTITY.LINT(LInt) -> LInt
    export IDENTITY.LREAL(LReal) -> LReal
    export IDENTITY.LWORD(LWord) -> LWord
    export IDENTITY.REAL(Real) -> Real
    export IDENTITY.SINT(SInt) -> SInt
    export IDENTITY.UDINT(UDInt) -> UDInt
    export IDENTITY.UINT(UInt) -> UInt
    export IDENTITY.ULINT(ULInt) -> ULInt
    export IDENTITY.USINT(USInt) -> USInt
    export IDENTITY.WORD(Word) -> Word
    export test() -> Int
    ");
}

#[rstest]
fn nested_any_calls_propagate_type(mut with_db: RootDatabase) {
    // ABS(ADD(1, 2)) - ADD infers INT from args, ABS infers INT from ADD result
    let source = r#"
FUNCTION ABS : ANY_NUM
VAR_INPUT IN : INTO(ABS); END_VAR
    {extern 'math' 'abs' (params IN) (result ABS)}
END_FUNCTION

FUNCTION ADD : ANY_MAGNITUDE
VAR_INPUT args : INTO(ADD)... END_VAR
END_FUNCTION

FUNCTION test : INT
    test := ABS(IN := ADD(1, 2));
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test() -> Int
    import math.abs.INT(Int) -> Int [from ABS]
    ");
}

#[rstest]
fn into_cross_constraint_same_type(mut with_db: RootDatabase) {
    // MOVE copies input to output - both must be the same type via INTO
    let source = r#"
FUNCTION MOVE : ANY
VAR_INPUT IN : INTO(MOVE); END_VAR
    MOVE := IN;
END_FUNCTION

FUNCTION test
VAR a : INT; b : REAL; END_VAR
    a := MOVE(IN := 42);
    b := MOVE(IN := 3.14);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export MOVE.INT(Int) -> Int
    export MOVE.REAL(Real) -> Real
    export test()
    ");
}

#[rstest]
fn conversion_wasm_intrinsic_no_type_ref(mut with_db: RootDatabase) {
    // Conversion functions use explicit wasm instructions - no type ref needed
    let source = r#"
FUNCTION INT_TO_REAL : REAL
VAR_INPUT IN : INT; END_VAR
    {wasm 'f32.convert_i32_s' (params IN) (result INT_TO_REAL)}
END_FUNCTION

FUNCTION REAL_TO_INT : INT
VAR_INPUT IN : REAL; END_VAR
    {wasm 'i32.trunc_sat_f32_s' (params IN) (result REAL_TO_INT)}
END_FUNCTION

FUNCTION test
VAR i : INT; r : REAL; END_VAR
    r := INT_TO_REAL(IN := 42);
    i := REAL_TO_INT(IN := 3.14);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export INT_TO_REAL(Int) -> Real
    export REAL_TO_INT(Real) -> Int
    export test()
    ");
}

#[rstest]
fn same_function_different_sites_deduplicates(mut with_db: RootDatabase) {
    // Calling ABS(INT) twice should only produce one ABS.INT export
    let source = r#"
FUNCTION ABS : ANY_NUM
VAR_INPUT IN : INTO(ABS); END_VAR
    {extern 'math' 'abs' (params IN) (result ABS)}
END_FUNCTION

FUNCTION test
VAR a : INT; b : INT; END_VAR
    a := ABS(IN := -1);
    b := ABS(IN := -2);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test()
    import math.abs.INT(Int) -> Int [from ABS]
    ");
}

#[rstest]
fn variadic_fold_multiple_args(mut with_db: RootDatabase) {
    // MUL(2, 3, 4) should fold into a single monomorphized MUL.INT
    let source = r#"
FUNCTION MUL : ANY_MAGNITUDE
VAR_INPUT args : INTO(MUL)... END_VAR
END_FUNCTION

FUNCTION test : INT
    test := MUL(2, 3, 4);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test() -> Int
    ");
}

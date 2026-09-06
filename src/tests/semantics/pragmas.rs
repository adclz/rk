use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

#[rstest]
fn valid_extern_pragma_minimal(mut with_db: RootDatabase) {
    let source = r#"
{extern 'math' 'abs'}
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The full interface: inputs become params, scalar outputs become results
/// (declaration order), the return type is the last result. Nothing is
/// restated in the pragma, so nothing can disagree with the declaration.
#[rstest]
fn valid_extern_pragma_with_outputs_and_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'rt' 'sample'}
FUNCTION sample : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT value : REAL; status : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// VAR_IN_OUT would hand the host a pointer into caller storage with a
/// mutation contract; an extern's interface is copies only.
#[rstest]
fn invalid_extern_in_out(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'fill'}
FUNCTION fill : INT
VAR_IN_OUT buf : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0243] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:4:12 ]
       |
     4 | VAR_IN_OUT buf : INT; END_VAR
       |            ^^^^|^^^^
       |                `------ VAR_IN_OUT 'buf' cannot cross a WASM import: an extern takes copies, not references
       |
       | Note: an extern FUNCTION receives VAR_INPUT copies and returns scalar VAR_OUTPUT results (the return value last)
    ---'
    ");
}

/// A struct/array/STRING output has no WASM result type to ride.
#[rstest]
fn invalid_extern_aggregate_output(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

{extern 'host' 'point'}
FUNCTION point : INT
VAR_OUTPUT p : Pt; s : STRING; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0243] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:6:12 ]
       |
     6 | VAR_OUTPUT p : Pt; s : STRING; END_VAR
       |            ^^^|^^
       |               `---- VAR_OUTPUT 'p' cannot be a WASM result: only scalar outputs cross an import
       |
       | Note: return scalars, or split the aggregate into scalar outputs
    ---'
    [E0243] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:6:20 ]
       |
     6 | VAR_OUTPUT p : Pt; s : STRING; END_VAR
       |                    ^^^^^|^^^^
       |                         `------ VAR_OUTPUT 's' cannot be a WASM result: only scalar outputs cross an import
       |
       | Note: return scalars, or split the aggregate into scalar outputs
    ---'
    ");
}

/// The import IS the body: statements on an extern FUNCTION are refused.
#[rstest]
fn invalid_extern_with_body(mut with_db: RootDatabase) {
    let source = r#"
{extern 'math' 'abs'}
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    test := x;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0243] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     test := x;
       |     ^^^^|^^^^
       |         `------ an extern FUNCTION has no statements
       |
       | Note: FUNCTIONs marked with {extern} act as external calls, they can not have a body
    ---'
    ");
}

/// Only a FUNCTION lowers to an import; anywhere else the pragma used to be
/// silently ignored, leaving the body it stood on empty.
#[rstest]
fn invalid_extern_outside_function(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'nope'}
FUNCTION_BLOCK Modbus
VAR_INPUT n : INT; END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0244] Error: extern pragma outside a FUNCTION
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {extern 'host' 'nope'}
       | ^^^^^^^^^^^|^^^^^^^^^^
       |            `------------ an {extern} pragma cannot be placed on a FUNCTION_BLOCK
       |
       | Note: {extern} pragmas can ony be used with FUNCTION
    ---'
    ");
}

#[rstest]
fn valid_test_pragma_referencing_normal_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_test_pragma_referencing_test_pou(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := test_helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_non_test_referencing_test_function(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_only : INT
END_FUNCTION

FUNCTION normal : INT
VAR x : INT; END_VAR
    x := test_only();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: access control violation
       ,-[ file:///test0.st:8:10 ]
       |
     8 |     x := test_only();
       |          ^^^^|^^^^
       |              `------ can not access test item 'test_only'
       |
       | Note: items marked with {test} can only be referenced from other {test} items
    ---'
    ");
}

// E0248/E0249: a {wasm} pragma is validated where it is written. An unknown
// name used to fall through to `unreachable` (or, on the conversion shape,
// to a silent identity), and a pragma outside a FUNCTION was silently
// dropped — all at exit 0.

#[rstest]
fn invalid_unknown_wasm_instruction(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION BOGUS : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            {wasm 'not.a.real.instruction' (params a b) (result BOGUS)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0248] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'not.a.real.instruction' (params a b) (result BOGUS)}
       |                   ^^^^^^^^^^^^|^^^^^^^^^^^
       |                               `------------- 'not.a.real.instruction' is not a wasm instruction this compiler emits
    ---'
    ");
}

#[rstest]
fn invalid_unknown_wasm_instruction_with_type_basis(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION BOGUS : INT
        VAR_INPUT a : INT; END_VAR
            {wasm a 'zorble' (params a) (result BOGUS)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0248] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:21 ]
       |
     4 |             {wasm a 'zorble' (params a) (result BOGUS)}
       |                     ^^^^|^^^
       |                         `----- 'zorble' is not a wasm instruction this compiler emits
    ---'
    ");
}

// E0253: a pragma's operands are the names it gives, resolved against the
// FUNCTION. An unknown one used to be ignored while the FUNCTION's own
// parameter list was lowered instead.

#[rstest]
fn invalid_wasm_operand_not_declared(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Root : REAL
        VAR_INPUT a : REAL; END_VAR
            {wasm 'f32.sqrt' (params b) (result Root)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0253] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:38 ]
       |
     4 |             {wasm 'f32.sqrt' (params b) (result Root)}
       |                                      |
       |                                      `-- 'b' is not a parameter, a local or the return of this FUNCTION
    ---'
    ");
}

#[rstest]
fn invalid_wasm_result_on_a_function_without_a_return(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Root
        VAR_INPUT a : REAL; END_VAR
            {wasm 'f32.sqrt' (params a) (result Root)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0253] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:49 ]
       |
     4 |             {wasm 'f32.sqrt' (params a) (result Root)}
       |                                                 ^^|^
       |                                                   `--- 'Root' is not a parameter, a local or the return of this FUNCTION
    ---'
    ");
}

#[rstest]
fn valid_wasm_pragmas_write_locals_in_any_case(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION round_sat : DINT
        VAR_INPUT IN : REAL; END_VAR
        VAR r : REAL; END_VAR
            {wasm 'f32.nearest' (params in) (result R)}
            {wasm 'i32.trunc_sat_f32_s' (params r) (result ROUND_SAT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// E0255: a pragma's operands must fit its instruction, lane for lane. The
// module validator used to be the first to say so, at load, from a compile
// that exited 0.

#[rstest]
fn invalid_wasm_operand_in_the_wrong_lane(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION NEAREST : DINT
        VAR_INPUT IN : DINT; END_VAR
            {wasm 'f32.nearest' (params IN) (result NEAREST)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0255] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'f32.nearest' (params IN) (result NEAREST)}
       |                   ^^^^^^|^^^^^^
       |                         `-------- 'f32.nearest' takes (f32) -> f32; this pragma gives it (IN: DINT (i32)) -> NEAREST: DINT (i32)
    ---'
    ");
}

#[rstest]
fn invalid_wasm_operand_count(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION ROOT : REAL
        VAR_INPUT a : REAL; b : REAL; END_VAR
            {wasm 'f32.sqrt' (params a b) (result ROOT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0255] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'f32.sqrt' (params a b) (result ROOT)}
       |                   ^^^^^|^^^^
       |                        `------ 'f32.sqrt' takes (f32) -> f32; this pragma gives it (a: REAL (f32), b: REAL (f32)) -> ROOT: REAL (f32)
    ---'
    ");
}

/// A conversion's written instruction must be the one its lanes name, even
/// though the cast road picks the op by the types: the text is what a
/// reader trusts.
#[rstest]
fn invalid_wasm_result_in_the_wrong_lane(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION TRUNCATE : REAL
        VAR_INPUT IN : REAL; END_VAR
            {wasm 'i32.trunc_sat_f32_s' (params IN) (result TRUNCATE)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0255] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'i32.trunc_sat_f32_s' (params IN) (result TRUNCATE)}
       |                   ^^^^^^^^^^|^^^^^^^^^^
       |                             `------------ 'i32.trunc_sat_f32_s' takes (f32) -> i32; this pragma gives it (IN: REAL (f32)) -> TRUNCATE: REAL (f32)
    ---'
    ");
}

#[rstest]
fn invalid_wasm_string_operand_on_a_numeric_instruction(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION LENGTH : DINT
        VAR_INPUT s : STRING; END_VAR
            {wasm 'str.byte_len' (params s) (result LENGTH)}
        END_FUNCTION

        FUNCTION ROOT : REAL
        VAR_INPUT s : STRING; END_VAR
            {wasm 'f32.sqrt' (params s) (result ROOT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0255] Error: invalid wasm pragma
       ,-[ file:///test0.st:9:19 ]
       |
     9 |             {wasm 'f32.sqrt' (params s) (result ROOT)}
       |                   ^^^^^|^^^^
       |                        `------ 'f32.sqrt' takes (f32) -> f32; this pragma gives it (s: STRING (i32, i32)) -> ROOT: REAL (f32)
    ---'
    ");
}

/// A type basis resolves to a lane the instruction may not exist for.
#[rstest]
fn invalid_wasm_type_basis_without_a_form(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION SHIFT : BOOL
        VAR_INPUT IN : BOOL; N : INT; END_VAR
            {wasm IN 'shl' (params IN N) (result SHIFT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0248] Error: invalid wasm pragma
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             {wasm IN 'shl' (params IN N) (result SHIFT)}
       |                      ^^|^^
       |                        `---- 'rk.shl1' is not a wasm instruction this compiler emits
    ---'
    ");
}

/// The shapes the library relies on all fit: a STRING producer, a STRING
/// consumer, a 64-bit rotate with an INT count, a widening with a basis.
#[rstest]
fn valid_wasm_signatures(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION JOIN : STRING
        VAR_INPUT a : STRING; b : STRING; END_VAR
            {wasm 'str.concat' (params a b) (result JOIN)}
        END_FUNCTION

        FUNCTION LENGTH : UDINT
        VAR_INPUT s : STRING; END_VAR
            {wasm 'str.byte_len' (params s) (result LENGTH)}
        END_FUNCTION

        FUNCTION ROTATE : LWORD
        VAR_INPUT IN : LWORD; N : INT; END_VAR
            {wasm IN 'rotl' (params IN N) (result ROTATE)}
        END_FUNCTION

        FUNCTION WIDEN : LREAL
        VAR_INPUT IN : REAL; END_VAR
            {wasm 'f64.promote_f32' (params IN) (result WIDEN)}
        END_FUNCTION

        FUNCTION ROOT : LREAL
        VAR_INPUT IN : LREAL; END_VAR
            {wasm IN 'sqrt' (params IN) (result ROOT)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_wasm_pragma_outside_function(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK FB
        VAR_INPUT a : INT; END_VAR
        VAR_OUTPUT o : INT; END_VAR
            {wasm 'i32.shl' (params a a) (result o)}
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0249] Error: invalid wasm pragma
       ,-[ file:///test0.st:5:19 ]
       |
     5 |             {wasm 'i32.shl' (params a a) (result o)}
       |                   ^^^^|^^^^
       |                       `------ a {wasm} body is only available on a FUNCTION; here the pragma would be silently dropped
    ---'
    ");
}

// The four families stay clean: a native name, a builtin, a bare op that
// prefixes on its type basis, and the conversion pseudo-names.
#[rstest]
fn valid_wasm_instruction_names(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION SHIFT : DINT
        VAR_INPUT a : DINT; n : DINT; END_VAR
            {wasm 'i32.shl' (params a n) (result SHIFT)}
        END_FUNCTION

        FUNCTION SINE : LREAL
        VAR_INPUT x : LREAL; END_VAR
            {wasm 'f64.sin' (params x) (result SINE)}
        END_FUNCTION

        FUNCTION SINB : LREAL
        VAR_INPUT x : LREAL; END_VAR
            {wasm x 'sin' (params x) (result SINB)}
        END_FUNCTION

        FUNCTION WIDEN : LINT
        VAR_INPUT x : DINT; END_VAR
            {wasm 'cast' (params x) (result WIDEN)}
        END_FUNCTION

        FUNCTION SAME : DINT
        VAR_INPUT x : DINT; END_VAR
            {wasm 'nop' (params x) (result SAME)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The return type is the import's LAST result, so the scalar rule that
/// governs VAR_OUTPUT governs it too. It is also the one place an aggregate
/// could still be written: checking only the sections let a STRING return
/// through to an assertion in codegen.
#[rstest]
fn invalid_extern_aggregate_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'greet'}
FUNCTION Greet : STRING
VAR_INPUT who : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0243] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:3:18 ]
       |
     3 | FUNCTION Greet : STRING
       |                  ^^^|^^
       |                     `---- the return type of 'Greet' can only be a scalar
    ---'
    ");
}

/// A scalar return is the ordinary case and stays legal.
#[rstest]
fn valid_extern_scalar_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'now'}
FUNCTION Now : LWORD
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// `{test}` is FUNCTION-only, like `{extern}`: the runner calls a `()` entry,
/// which no other POU kind has. A helper that only tests use is hidden with
/// `FUNCTION PRIVATE`, not marked as a test.
#[rstest]
fn invalid_test_pragma_outside_a_function(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION_BLOCK fb
END_FUNCTION_BLOCK

{test}
PROGRAM prog
END_PROGRAM

CLASS C
    {test}
    METHOD m : INT
        m := 1;
    END_METHOD
END_CLASS

{test}
FUNCTION t : INT
    t := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0252] Error: test pragma outside a FUNCTION
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {test}
       | ^^^|^^
       |    `---- a {test} pragma cannot be placed on a FUNCTION_BLOCK
    ---'
    [E0252] Error: test pragma outside a FUNCTION
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     {test}
        |     ^^^|^^
        |        `---- a {test} pragma cannot be placed on a METHOD
    ----'
    [E0252] Error: test pragma outside a FUNCTION
       ,-[ file:///test0.st:6:1 ]
       |
     6 | {test}
       | ^^^|^^
       |    `---- a {test} pragma cannot be placed on a PROGRAM
    ---'
    ");
}


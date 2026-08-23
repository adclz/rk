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

#[rstest]
fn invalid_non_test_referencing_test_program(mut with_db: RootDatabase) {
    let source = r#"
{test}
PROGRAM test_prog
END_PROGRAM

FUNCTION normal : INT
VAR x : INT; END_VAR
END_FUNCTION"#;

    // Programs are not visible to functions anyway (only config scopes),
    // so no E0404 emitted here - just verifying no crash.
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
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

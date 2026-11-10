// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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

/// The core interface a WASI import keeps: inputs become params, scalar
/// outputs become results (declaration order), the return type is the last
/// result. Nothing is restated in the pragma, so nothing can disagree with
/// the declaration. The interface is made up: any in the `wasi` namespace
/// keeps the core rules, and WASI's WIT decides at the build.
#[rstest]
fn valid_wasi_extern_with_outputs_and_return(mut with_db: RootDatabase) {
    let source = r#"
{extern 'wasi:test/rt@0.1.0' 'sample'}
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
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:4:12 ]
       |
     4 | VAR_IN_OUT buf : INT; END_VAR
       |            ^|^
       |             `--- VAR_IN_OUT 'buf' cannot cross a WASM import: an extern takes copies, not references
       |
       | Note: an extern FUNCTION receives copies of its VAR_INPUT
    ---'
    ");
}

/// A struct/array/STRING output has no WASM result type to ride, on a WASI
/// import too, which keeps the core rules (the interface is made up).
#[rstest]
fn invalid_extern_aggregate_output(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

{extern 'wasi:test/host@0.1.0' 'point'}
FUNCTION point : INT
VAR_OUTPUT p : Pt; s : STRING; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:6:12 ]
       |
     6 | VAR_OUTPUT p : Pt; s : STRING; END_VAR
       |            |
       |            `-- VAR_OUTPUT 'p' cannot be a WASM result: only scalar outputs cross an import
       |
       | Note: return scalars, or split the aggregate into scalar outputs
    ---'
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:6:20 ]
       |
     6 | VAR_OUTPUT p : Pt; s : STRING; END_VAR
       |                    |
       |                    `-- VAR_OUTPUT 's' cannot be a WASM result: only scalar outputs cross an import
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
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     test := x;
       |     ^^^^|^^^^
       |         `------ an extern FUNCTION has no statements
       |
       | Note: an {extern} FUNCTION is an import: the host has its body
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
    [E1501] Error: extern pragma outside a FUNCTION
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {extern 'host' 'nope'}
       | ^^^^^^^^^^^|^^^^^^^^^^
       |            `------------ an {extern} pragma cannot be placed on a FUNCTION_BLOCK
       |
       | Note: only a FUNCTION can be {extern}
    ---'
    ");
}

/// An import from a module other than WASI's is typed in WIT from its
/// declaration: every scalar has a WIT type, an enum and a subrange the one
/// they are stored as, a STRING input is a `string`, and the return value is
/// the one result.
#[rstest]
fn valid_extern_from_a_host_module(mut with_db: RootDatabase) {
    let source = r#"
TYPE Mode : (Off, Slow, Fast); END_TYPE
TYPE Percent : INT (0..100); END_TYPE

{extern 'modbus' 'write-numbers'}
FUNCTION WriteNumbers : BOOL
VAR_INPUT
    flag : BOOL;
    s8 : SINT; s16 : INT; s32 : DINT; s64 : LINT;
    u8 : USINT; u16 : UINT; u32 : UDINT; u64 : ULINT;
    f32 : REAL; f64 : LREAL;
END_VAR
END_FUNCTION

{extern 'modbus' 'write-the-rest'}
FUNCTION WriteTheRest : CHAR
VAR_INPUT
    b8 : BYTE; b16 : WORD; b32 : DWORD; b64 : LWORD;
    letter : CHAR;
    text : STRING[20];
    mode : Mode;
    level : Percent;
END_VAR
END_FUNCTION

{extern 'modbus' 'read-clock'}
FUNCTION ReadClock : LDT
VAR_INPUT t : TIME; lt : LTIME; d : DATE; ld : LDATE; time_of_day : TOD; long_time_of_day : LTOD; stamp : DT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A module is a WIT name or an interface, with or without its version,
/// and a name is a WIT name: words of lowercase letters or of capitals,
/// with digits, a word after the first may start with a digit.
#[rstest]
fn valid_extern_wit_names(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus-rtu' 'read-coils'}
FUNCTION A : INT
END_FUNCTION

{extern 'acme:modbus/rtu' 'HTTP-get'}
FUNCTION B : INT
END_FUNCTION

{extern 'acme:modbus/RTU@1.2.3-beta.1+build.5' 'axis-2'}
FUNCTION C : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A module is written as WIT writes it, never converted: the host defines
/// the name the declaration gives. What would be a WIT name is suggested.
#[rstest]
fn invalid_extern_module_not_a_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'my_host' 'now'}
FUNCTION A : INT
END_FUNCTION

{extern 'Modbus' 'now'}
FUNCTION B : INT
END_FUNCTION

{extern 'acme:io@1.0.0' 'now'}
FUNCTION C : INT
END_FUNCTION

{extern 'Acme:io/poll' 'now'}
FUNCTION D : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {extern 'my_host' 'now'}
       |         ^^^^|^^^^
       |             `------ 'my_host' is not a WIT name
       |
       | Help: write 'my-host'
       |
       | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:6:9 ]
       |
     6 | {extern 'Modbus' 'now'}
       |         ^^^^|^^^
       |             `----- 'Modbus' is not a WIT name
       |
       | Help: write 'modbus'
       |
       | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ---'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:10:9 ]
        |
     10 | {extern 'acme:io@1.0.0' 'now'}
        |         ^^^^^^^|^^^^^^^
        |                `--------- 'acme:io@1.0.0' is not an interface's name
        |
        | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ----'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:14:9 ]
        |
     14 | {extern 'Acme:io/poll' 'now'}
        |         ^^^^^^^|^^^^^^
        |                `-------- 'Acme:io/poll' is not an interface's name
        |
        | Note: an import's module is a WIT name, like `modbus`, or an interface, like `acme:modbus/rtu@1.0.0`
    ----'
    ");
}

/// A second namespace or a second interface is a nested name, which the
/// component model's tools do not take yet.
#[rstest]
fn invalid_extern_module_nested(mut with_db: RootDatabase) {
    let source = r#"
{extern 'acme:modbus:rtu/read' 'now'}
FUNCTION A : INT
END_FUNCTION

{extern 'acme:modbus/rtu/read' 'now'}
FUNCTION B : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {extern 'acme:modbus:rtu/read' 'now'}
       |         ^^^^^^^^^^^|^^^^^^^^^^
       |                    `------------ 'acme:modbus:rtu/read' is a nested name
       |
       | Note: an interface is `namespace:package/interface`, and the component model's tools take no nested name yet
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:6:9 ]
       |
     6 | {extern 'acme:modbus/rtu/read' 'now'}
       |         ^^^^^^^^^^^|^^^^^^^^^^
       |                    `------------ 'acme:modbus/rtu/read' is a nested name
       |
       | Note: an interface is `namespace:package/interface`, and the component model's tools take no nested name yet
    ---'
    ");
}

/// An interface's version is a semantic version, as the component model
/// reads one.
#[rstest]
fn invalid_extern_module_version(mut with_db: RootDatabase) {
    let source = r#"
{extern 'acme:modbus/rtu@1' 'now'}
FUNCTION A : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:9 ]
       |
     2 | {extern 'acme:modbus/rtu@1' 'now'}
       |         ^^^^^^^^^|^^^^^^^^^
       |                  `----------- '1' is not a semantic version
       |
       | Note: an interface's version is `MAJOR.MINOR.PATCH`, like `1.0.0`
    ---'
    ");
}

/// A function's name is a WIT name, WASI's too.
#[rstest]
fn invalid_extern_function_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus' 'read_coils'}
FUNCTION A : INT
END_FUNCTION

{extern 'modbus' 'ReadCoils'}
FUNCTION B : INT
END_FUNCTION

{extern 'wasi:clocks/monotonic-clock@0.3.0' 'sqrt.REAL'}
FUNCTION C : INT
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:2:18 ]
       |
     2 | {extern 'modbus' 'read_coils'}
       |                  ^^^^^^|^^^^^
       |                        `------- 'read_coils' is not a WIT name
       |
       | Help: write 'read-coils'
       |
       | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:6:18 ]
       |
     6 | {extern 'modbus' 'ReadCoils'}
       |                  ^^^^^|^^^^^
       |                       `------- 'ReadCoils' is not a WIT name
       |
       | Help: write 'read-coils'
       |
       | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ---'
    [E1512] Error: import name not a WIT name
        ,-[ file:///test0.st:10:45 ]
        |
     10 | {extern 'wasi:clocks/monotonic-clock@0.3.0' 'sqrt.REAL'}
        |                                             ^^^^^|^^^^^
        |                                                  `------- 'sqrt.REAL' is not a WIT name
        |
        | Note: an import's name is written as WIT writes one, words of lowercase letters and digits joined by '-'
    ----'
    ");
}

/// A parameter's WIT name is its name in lowercase with each `_` written
/// `-`: a name that leaves a word empty, or has a letter outside ASCII, has
/// none.
#[rstest]
fn invalid_extern_parameter_without_a_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus' 'write'}
FUNCTION A : INT
VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |           ^|
       |            `-- 'x_' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:21 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |                     ^|
       |                      `-- '_y' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:31 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |                               ^^|^
       |                                 `--- 'a__b' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:43 ]
       |
     4 | VAR_INPUT x_ : INT; _y : INT; a__b : INT; température : REAL; END_VAR
       |                                           ^^^^^|^^^^^
       |                                                `------- 'température' has no WIT name
       |
       | Note: a parameter's WIT name is its name in lowercase with each '_' written '-', and a WIT name has no empty word and no letter outside ASCII
    ---'
    ");
}

/// The component model tells two names apart without their `-`, so `ab`
/// and `a_b` are one parameter to it.
#[rstest]
fn invalid_extern_parameters_with_one_wit_name(mut with_db: RootDatabase) {
    let source = r#"
{extern 'modbus' 'write'}
FUNCTION A : INT
VAR_INPUT ab : INT; a_b : INT; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1512] Error: import name not a WIT name
       ,-[ file:///test0.st:4:21 ]
       |
     4 | VAR_INPUT ab : INT; a_b : INT; END_VAR
       |           ^|        ^|^
       |            `------------- 'ab' is declared here
       |                      |
       |                      `--- 'a_b' cannot be told from 'ab' in WIT
       |
       | Note: the component model compares two names without their '-'
    ---'
    ");
}

/// A WIT function returns one value: an import from a module other than
/// WASI's has no VAR_OUTPUT, scalar or not.
#[rstest]
fn invalid_extern_output_from_a_host_module(mut with_db: RootDatabase) {
    let source = r#"
{extern 'rt' 'sample'}
FUNCTION sample : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT value : REAL; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:5:12 ]
       |
     5 | VAR_OUTPUT value : REAL; END_VAR
       |            ^^|^^
       |              `---- VAR_OUTPUT 'value' cannot be a result of the import: a WIT function returns one value
       |
       | Note: an {extern} FUNCTION from a module other than WASI's returns its return value only
    ---'
    ");
}

/// The component model passes no address: a REF_TO, a STRUCT, an ARRAY and
/// an instance are refused as the inputs of an import from a module other
/// than WASI's.
#[rstest]
fn invalid_extern_reference_and_aggregate_inputs(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Counter
VAR count : INT; END_VAR
END_FUNCTION_BLOCK

{extern 'host' 'send'}
FUNCTION Send : INT
VAR_INPUT
    r : REF_TO INT;
    p : Pt;
    a : ARRAY[0..3] OF BYTE;
    c : Counter;
END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an extern FUNCTION
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     r : REF_TO INT;
        |     |
        |     `-- VAR_INPUT 'r' cannot cross a component import: an address in the module means nothing to the host
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    [E1502] Error: not representable on an extern FUNCTION
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     p : Pt;
        |     |
        |     `-- VAR_INPUT 'p' cannot cross a component import: only a scalar or a STRING can
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    [E1502] Error: not representable on an extern FUNCTION
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     a : ARRAY[0..3] OF BYTE;
        |     |
        |     `-- VAR_INPUT 'a' cannot cross a component import: only a scalar or a STRING can
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    [E1502] Error: not representable on an extern FUNCTION
        ,-[ file:///test0.st:14:5 ]
        |
     14 |     c : Counter;
        |     |
        |     `-- VAR_INPUT 'c' cannot cross a component import: only a scalar or a STRING can
        |
        | Note: an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs
    ----'
    ");
}

/// An import takes 16 values, a STRING counting two: past them the
/// component model passes the values in memory, which a call does not.
#[rstest]
fn invalid_extern_more_than_16_values(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'sixteen'}
FUNCTION Sixteen : INT
VAR_INPUT
    s1 : STRING; s2 : STRING; s3 : STRING; s4 : STRING;
    s5 : STRING; s6 : STRING; s7 : STRING; s8 : STRING;
END_VAR
END_FUNCTION

{extern 'host' 'seventeen'}
FUNCTION Seventeen : INT
VAR_INPUT
    s1 : STRING; s2 : STRING; s3 : STRING; s4 : STRING;
    s5 : STRING; s6 : STRING; s7 : STRING; s8 : STRING;
    n : INT;
END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an extern FUNCTION
        ,-[ file:///test0.st:15:5 ]
        |
     15 |     n : INT;
        |     |
        |     `-- VAR_INPUT 'n' cannot cross a component import: the import would take more than 16 values
        |
        | Note: a component import takes 16 values at most, a STRING counting two
    ----'
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

/// A test cannot call another test either. The runner is the only caller: a
/// `{test}` FUNCTION is emitted with the runner's signature, not the one it
/// declares, so a call produced a module that failed to load, or handed back
/// the runner's pass/fail code in place of the declared value.
#[rstest]
fn invalid_test_pragma_referencing_test_pou(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION test_helper : INT
END_FUNCTION

{test}
FUNCTION my_test : INT
VAR x : INT; END_VAR
    x := test_helper();
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Error: access to a test-only item
       ,-[ file:///test0.st:9:10 ]
       |
     9 |     x := test_helper();
       |          ^^^^^|^^^^^
       |               `------- cannot access test item 'test_helper'
       |
       | Help: for code shared between tests, write a FUNCTION without the pragma
       |
       | Note: a {test} FUNCTION is the test runner's entry point, not a callable
    ---'
    ");
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
    [E1007] Error: access to a test-only item
       ,-[ file:///test0.st:8:10 ]
       |
     8 |     x := test_only();
       |          ^^^^|^^^^
       |              `------ cannot access test item 'test_only'
       |
       | Help: for code shared between tests, write a FUNCTION without the pragma
       |
       | Note: a {test} FUNCTION is the test runner's entry point, not a callable
    ---'
    ");
}

// E1505/E1504: a {wasm} pragma is validated where it is written. An unknown
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
    [E1505] Error: unknown wasm instruction
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
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:21 ]
       |
     4 |             {wasm a 'zorble' (params a) (result BOGUS)}
       |                     ^^^^|^^^
       |                         `----- 'zorble' is not a wasm instruction this compiler emits
    ---'
    ");
}

// E1506: a pragma's operands are the names it gives, resolved against the
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
    [E1506] Error: wasm operand not declared
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
    [E1506] Error: wasm operand not declared
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

// E1507: a pragma's operands must fit its instruction, lane for lane. The
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
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'f32.nearest' (params IN) (result NEAREST)}
       |                   ^^^^^^|^^^^^^
       |                         `-------- 'f32.nearest' takes (f32) -> f32, not (IN: DINT (i32)) -> NEAREST: DINT (i32)
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
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'f32.sqrt' (params a b) (result ROOT)}
       |                   ^^^^^|^^^^
       |                        `------ 'f32.sqrt' takes (f32) -> f32, not (a: REAL (f32), b: REAL (f32)) -> ROOT: REAL (f32)
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
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:4:19 ]
       |
     4 |             {wasm 'i32.trunc_sat_f32_s' (params IN) (result TRUNCATE)}
       |                   ^^^^^^^^^^|^^^^^^^^^^
       |                             `------------ 'i32.trunc_sat_f32_s' takes (f32) -> i32, not (IN: REAL (f32)) -> TRUNCATE: REAL (f32)
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
    [E1507] Error: wasm operands of the wrong type
       ,-[ file:///test0.st:9:19 ]
       |
     9 |             {wasm 'f32.sqrt' (params s) (result ROOT)}
       |                   ^^^^^|^^^^
       |                        `------ 'f32.sqrt' takes (f32) -> f32, not (s: STRING (i32, i32)) -> ROOT: REAL (f32)
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
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             {wasm IN 'shl' (params IN N) (result SHIFT)}
       |                      ^^|^^
       |                        `---- 'rk.shl1' is not a wasm instruction this compiler emits
    ---'
    ");
}

/// One byte has no order, and a REAL has no byte swap: `rk.bswap8` and
/// `f32.bswap` are no instructions.
#[rstest]
fn invalid_wasm_byte_swap_without_a_form(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION SWAP8 : BYTE
        VAR_INPUT IN : BYTE; END_VAR
            {wasm IN 'bswap' (params IN) (result SWAP8)}
        END_FUNCTION

        FUNCTION SWAPR : REAL
        VAR_INPUT IN : REAL; END_VAR
            {wasm IN 'bswap' (params IN) (result SWAPR)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             {wasm IN 'bswap' (params IN) (result SWAP8)}
       |                      ^^^|^^^
       |                         `----- 'rk.bswap8' is not a wasm instruction this compiler emits
    ---'
    [E1505] Error: unknown wasm instruction
       ,-[ file:///test0.st:9:22 ]
       |
     9 |             {wasm IN 'bswap' (params IN) (result SWAPR)}
       |                      ^^^|^^^
       |                         `----- 'f32.bswap' is not a wasm instruction this compiler emits
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

        FUNCTION SWAP : LWORD
        VAR_INPUT IN : LWORD; END_VAR
            {wasm IN 'bswap' (params IN) (result SWAP)}
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
    [E1504] Error: wasm pragma outside a FUNCTION
       ,-[ file:///test0.st:5:19 ]
       |
     5 |             {wasm 'i32.shl' (params a a) (result o)}
       |                   ^^^^|^^^^
       |                       `------ a {wasm} body is only available on a FUNCTION
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
    [E1502] Error: not representable on an extern FUNCTION
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
    [E1503] Error: test pragma outside a FUNCTION
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {test}
       | ^^^|^^
       |    `---- a {test} pragma cannot be placed on a FUNCTION_BLOCK
    ---'
    [E1503] Error: test pragma outside a FUNCTION
       ,-[ file:///test0.st:6:1 ]
       |
     6 | {test}
       | ^^^|^^
       |    `---- a {test} pragma cannot be placed on a PROGRAM
    ---'
    [E1503] Error: test pragma outside a FUNCTION
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     {test}
        |     ^^^|^^
        |        `---- a {test} pragma cannot be placed on a METHOD
    ----'
    ");
}

/// `{export}` makes a FUNCTION a WASM export, and nothing else does.
#[rstest]
fn valid_export_pragma(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Scale : REAL
VAR_INPUT raw : INT; gain : REAL; END_VAR
VAR_IN_OUT total : REAL; END_VAR
VAR_OUTPUT clamped : BOOL; END_VAR
    Scale := raw * gain;
END_FUNCTION

{once}
{export}
FUNCTION Setup : INT
    Setup := 1;
END_FUNCTION

NAMESPACE Plant
    {export}
    FUNCTION Reset : INT
        Reset := 0;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A host can only call a FUNCTION. A PROGRAM is exported for the schedule
/// already, and an FB or a METHOD needs an instance the host does not have.
#[rstest]
fn invalid_export_pragma_outside_a_function(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION_BLOCK fb
END_FUNCTION_BLOCK

{export}
PROGRAM prog
END_PROGRAM

FUNCTION_BLOCK holder
    {export}
    METHOD m : INT
        m := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1508] Error: export pragma outside a FUNCTION
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- an {export} pragma cannot be placed on a FUNCTION_BLOCK
       |
       | Note: only a FUNCTION can be {export}
    ---'
    [E1508] Error: export pragma outside a FUNCTION
       ,-[ file:///test0.st:6:1 ]
       |
     6 | {export}
       | ^^^^|^^^
       |     `----- an {export} pragma cannot be placed on a PROGRAM
       |
       | Note: only a FUNCTION can be {export}
    ---'
    [E1508] Error: export pragma outside a FUNCTION
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     {export}
        |     ^^^^|^^^
        |         `----- an {export} pragma cannot be placed on a METHOD
        |
        | Note: only a FUNCTION can be {export}
    ----'
    ");
}

/// An import has no body to export, and a test is exported for the runner
/// under the same name, in a debug build only.
#[rstest]
fn invalid_export_on_an_extern_or_a_test(mut with_db: RootDatabase) {
    let source = r#"
{export}
{extern 'host' 'now'}
FUNCTION HostNow : LINT
END_FUNCTION

{test}
{export}
FUNCTION checks_something
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'HostNow' cannot be exported: it is an {extern} FUNCTION
       |
       | Note: an {extern} FUNCTION is an import, it has no body to export
    ---'
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:8:1 ]
       |
     8 | {export}
       | ^^^^|^^^
       |     `----- 'checks_something' cannot be exported: it is a {test} FUNCTION
       |
       | Note: a {test} FUNCTION is already exported for `rk test` and left out of a release build
    ---'
    ");
}

/// One declaration the lowering turns into several functions has no single
/// export to give: `drive$@Worker`, `sum_all$3`.
#[rstest]
fn invalid_export_on_a_function_lowered_more_than_once(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IWork
    METHOD Run : INT END_METHOD
END_INTERFACE

{export}
FUNCTION drive : INT
VAR_IN_OUT dev : IWork; END_VAR
    drive := dev.Run();
END_FUNCTION

{export}
FUNCTION sum_all : INT
VAR_INPUT args : INT...; END_VAR
    sum_all := ...args+;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:6:1 ]
       |
     6 | {export}
       | ^^^^|^^^
       |     `----- 'drive' cannot be exported: it takes an interface
       |
       | Note: it is compiled once per implementation it is called with, so there is no single function to export
    ---'
    [E1509] Error: FUNCTION not exportable
        ,-[ file:///test0.st:12:1 ]
        |
     12 | {export}
        | ^^^^|^^^
        |     `----- 'sum_all' cannot be exported: it is variadic
        |
        | Note: it is compiled once per number of arguments it is called with, so there is no single function to export
    ----'
    ");
}

/// An overloaded FUNCTION's symbol carries its signature (`Twice$INT`), and a
/// host finds an export by its name. Only the marked overload is refused.
#[rstest]
fn invalid_export_on_an_overloaded_function(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION Twice : INT
VAR_INPUT a : INT; END_VAR
    Twice := a * 2;
END_FUNCTION

FUNCTION Twice : REAL
VAR_INPUT a : REAL; END_VAR
    Twice := a * 2.0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'Twice' cannot be exported: it is overloaded
       |
       | Note: the module finds an export by its name, which several FUNCTIONs share
    ---'
    ");
}

/// A FUNCTION exported under a name the module exports itself would make a
/// module with two exports of that name, which does not load.
#[rstest]
fn export_named_like_a_module_export_is_refused(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION memory : INT
    memory := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'memory' cannot be exported: it has the name of an export the module makes
       |
       | Note: the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names
    ---'
    ");
}

/// `cabi_realloc` is what a host places a list with when it calls the
/// control interface: a FUNCTION exported under that name would be a second
/// export of it.
#[rstest]
fn export_named_like_the_list_allocator_is_refused(mut with_db: RootDatabase) {
    let source = r#"
{export}
FUNCTION cabi_realloc : INT
    cabi_realloc := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {export}
       | ^^^^|^^^
       |     `----- 'cabi_realloc' cannot be exported: it has the name of an export the module makes
       |
       | Note: the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names
    ---'
    ");
}

/// A `{test}` is exported under its name in a debug build.
#[rstest]
fn test_named_like_a_module_export_is_refused(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION __init
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {test}
       | ^^^|^^
       |    `---- '__init' cannot be exported: it has the name of an export the module makes
       |
       | Note: the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names
    ---'
    ");
}

/// In a namespace the export is `Io.memory`, which the module does not use.
#[rstest]
fn export_in_a_namespace_may_take_a_module_export_name(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Io
    {export}
    FUNCTION memory : INT
        memory := 1;
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A pragma rk has no rule for is E1510 wherever it stands: above a POU,
/// in a declaration section, after a type, in a body. Inside a section it
/// used to be read and dropped without a word. It is read to its `}`, so a
/// `//` or a `}` inside a quoted string is text: the `//` used to start a
/// line comment that ate the `}`. A known name in capitals is pointed at
/// its lowercase spelling. A `{` in a string, a comment or a known pragma
/// opens no pragma, and one inside a pragma's text opens no second one.
#[rstest]
fn invalid_unknown_pragmas(mut with_db: RootDatabase) {
    let source = r#"
{attribute 'qualified_only'}
FUNCTION_BLOCK Fb
VAR
    {attribute 'see http://x'}
    a : INT;
    b : INT {attribute 'a}b'};
END_VAR
    {pack_mode := '1'}
    a := 1;
END_FUNCTION_BLOCK

{TEST}
FUNCTION t
END_FUNCTION

{info = 'a {braced} note'}
FUNCTION u : STRING
    u := '{not a pragma}'; (* {nor this} *) // {nor this}
    {attribute '{u}'}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:2:1 ]
       |
     2 | {attribute 'qualified_only'}
       | ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
       |               `--------------- no pragma named 'attribute'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     {attribute 'see http://x'}
       |     ^^^^^^^^^^^^^|^^^^^^^^^^^^
       |                  `-------------- no pragma named 'attribute'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     b : INT {attribute 'a}b'};
       |             ^^^^^^^^|^^^^^^^^
       |                     `---------- no pragma named 'attribute'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
       ,-[ file:///test0.st:9:5 ]
       |
     9 |     {pack_mode := '1'}
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- no pragma named 'pack_mode'
       |
       | Help: remove it
       |
       | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ---'
    [E1510] Error: unknown pragma
        ,-[ file:///test0.st:13:1 ]
        |
     13 | {TEST}
        | ^^^|^^
        |    `---- no pragma named 'TEST'
        |
        | Help: write it in lowercase, {test}
        |
        | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ----'
    [E1510] Error: unknown pragma
        ,-[ file:///test0.st:20:5 ]
        |
     20 |     {attribute '{u}'}
        |     ^^^^^^^^|^^^^^^^^
        |             `---------- no pragma named 'attribute'
        |
        | Help: remove it
        |
        | Note: the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}
    ----'
    ");
}

/// A known pragma where its rule cannot stand is a syntax error: `{export}`
/// after a type, and `{ test }`, which `{test}` is written without spaces.
/// Both were read as an unknown pragma and dropped, so the FUNCTION was
/// quietly no test.
#[rstest]
fn invalid_known_pragma_out_of_place(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Row : ARRAY[0..1] OF INT {export};
END_TYPE

{ test }
FUNCTION t
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0001] Error: syntax error
       ,-[ file:///test0.st:3:30 ]
       |
     3 |     Row : ARRAY[0..1] OF INT {export};
       |                              ^^^^|^^^
       |                                  `----- unexpected token(s): '{export}'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:6:1 ]
       |
     6 | { test }
       | ^^^^|^^^
       |     `----- unexpected token(s): '{ test }'
    ---'
    ");
}

/// A variadic parameter has one copy per argument count, and an import has one
/// signature: this passed `rk check`, and the build looked for a copy.
#[rstest]
fn invalid_extern_variadic(mut with_db: RootDatabase) {
    let source = r#"
{extern 'host' 'sum'}
FUNCTION sum : INT
VAR_INPUT values : INT...; END_VAR
END_FUNCTION

PROGRAM Main
VAR x : INT; END_VAR
    x := sum(1, 2);
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT values : INT...; END_VAR
       |           ^^^|^^
       |              `---- VAR_INPUT 'values' cannot cross a WASM import: a variadic parameter has the argument count of each call
       |
       | Note: a WASM import has one signature
    ---'
    ");
}

/// The runner calls a test with no arguments, and its wrapper has no parameters
/// to read. A scalar read a scratch local, what goes by address wrote near address
/// 0, a REAL made the module invalid, and a test taking a variadic or an interface
/// had no copy, so it never ran.
#[rstest]
fn invalid_test_with_a_parameter(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE Itf
    METHOD Run : INT
    END_METHOD
END_INTERFACE

{test}
FUNCTION scalar
VAR_INPUT a : INT; END_VAR
END_FUNCTION

{test}
FUNCTION float
VAR_INPUT a : REAL; END_VAR
END_FUNCTION

{test}
FUNCTION by_address
VAR_IN_OUT a : INT; END_VAR
END_FUNCTION

{test}
FUNCTION output
VAR_OUTPUT a : REAL; END_VAR
END_FUNCTION

{test}
FUNCTION pack
VAR_INPUT a : INT...; END_VAR
END_FUNCTION

{test}
FUNCTION implementation
VAR_INPUT a : Itf; END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1511] Error: test FUNCTION with a parameter
       ,-[ file:///test0.st:9:11 ]
       |
     9 | VAR_INPUT a : INT; END_VAR
       |           |
       |           `-- the {test} FUNCTION cannot take the parameter 'a'
       |
       | Help: declare it in VAR, or call a FUNCTION taking it from the test
       |
       | Note: the runner calls a test with no arguments
    ---'
    [E1511] Error: test FUNCTION with a parameter
        ,-[ file:///test0.st:14:11 ]
        |
     14 | VAR_INPUT a : REAL; END_VAR
        |           |
        |           `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test FUNCTION with a parameter
        ,-[ file:///test0.st:19:12 ]
        |
     19 | VAR_IN_OUT a : INT; END_VAR
        |            |
        |            `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test FUNCTION with a parameter
        ,-[ file:///test0.st:24:12 ]
        |
     24 | VAR_OUTPUT a : REAL; END_VAR
        |            |
        |            `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test FUNCTION with a parameter
        ,-[ file:///test0.st:29:11 ]
        |
     29 | VAR_INPUT a : INT...; END_VAR
        |           |
        |           `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    [E1511] Error: test FUNCTION with a parameter
        ,-[ file:///test0.st:34:11 ]
        |
     34 | VAR_INPUT a : Itf; END_VAR
        |           |
        |           `-- the {test} FUNCTION cannot take the parameter 'a'
        |
        | Help: declare it in VAR, or call a FUNCTION taking it from the test
        |
        | Note: the runner calls a test with no arguments
    ----'
    ");
}

/// A result is a local of the test, which nothing reads.
#[rstest]
fn valid_test_with_a_result(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT a : INT; b : REAL; END_STRUCT; END_TYPE

{test}
FUNCTION float : REAL
    float := 1.0;
END_FUNCTION

{test}
FUNCTION aggregate : Pair
    aggregate.b := 1.0;
END_FUNCTION

{test}
FUNCTION text : STRING
    text := 'a';
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

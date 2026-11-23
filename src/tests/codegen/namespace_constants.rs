// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `VAR_CONSTANT`: a namespace's constant whose value is one expression is
//! that value wherever a body reads it; one of a STRUCT or an ARRAY is held
//! once in static memory, which `__init` fills.

use crate::tests::codegen::{
    TestPlc, compile_tests, compile_to_mir_and_wasm, compile_to_wasm, execute_wasm, run_tests,
    with_db,
};
use debug_format::test_report::Status;
use debug_format::{DebugInfo, VarValue};
use rstest::*;

/// FUNCTIONs that read constants as a body names them: of the root
/// namespace, qualified, through USING, inside their namespace, of each
/// elementary kind.
const READS: &str = "
VAR_CONSTANT
    ROWS : INT := 10;
    SCALE : REAL := 2.5;
    WIDE : LINT := 5_000_000_000;
    ON : BOOL := TRUE;
END_VAR

NAMESPACE Limits
    VAR_CONSTANT
        HIGH : INT := 6;
        TWICE : INT := HIGH * 2;
    END_VAR
    FUNCTION Inside : INT
        Inside := TWICE;
    END_FUNCTION
END_NAMESPACE

FUNCTION Sum : INT
    Sum := ROWS + Limits.HIGH;
END_FUNCTION

FUNCTION Imported : INT
USING Limits;
    Imported := HIGH + TWICE;
END_FUNCTION

FUNCTION Scaled : REAL
    Scaled := SCALE * 2.0;
END_FUNCTION

FUNCTION Large : LINT
    Large := WIDE + 1;
END_FUNCTION

FUNCTION Switched : BOOL
    Switched := ON AND NOT FALSE;
END_FUNCTION
";

/// Each read is the constant's value.
#[rstest]
fn a_body_reads_a_constant_s_value(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, READS);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "Sum", ()), 16);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "Imported", ()), 18);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "Limits.Inside", ()), 12);
    assert_eq!(execute_wasm::<(), f32>(&wasm, "Scaled", ()), 5.0);
    assert_eq!(execute_wasm::<(), i64>(&wasm, "Large", ()), 5_000_000_001);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "Switched", ()), 1);
}

/// A PROGRAM that reads constants of aggregate types and a STRING: a field,
/// an element at a constant and at a computed subscript, the whole value
/// copied, the STRING compared, a field of an element; then a constant of a
/// namespace, qualified, through USING and from inside its namespace. Their
/// storage holds their values once `__init` has run, as a host runs it first.
const AGGREGATES: &str = "
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

VAR_CONSTANT
    NAME : STRING := 'rk!';
    ORIGIN : Point := (x := 1, y := 2);
    PRIMES : ARRAY[0..2] OF INT := [2, 3, 5];
    TABLE : ARRAY[0..1] OF Point := [(x := 5, y := 6), (x := 7, y := 8)];
END_VAR

NAMESPACE Shapes
    VAR_CONSTANT
        CORNER : Point := (x := 3, y := 4);
    END_VAR
    FUNCTION Inside : INT
        Inside := CORNER.y;
    END_FUNCTION
END_NAMESPACE

FUNCTION Imported : INT
USING Shapes;
    Imported := CORNER.x;
END_FUNCTION

PROGRAM P
VAR
    i : INT := 2;
    p : Point;
    q : Point;
    s : STRING;
    field : INT;
    element : INT;
    computed : INT;
    whole : INT;
    named : BOOL;
    nested : INT;
    qualified : INT;
    copied : INT;
    through_using : INT;
    inside : INT;
END_VAR
    field := ORIGIN.y;
    element := PRIMES[1];
    computed := PRIMES[i];
    p := ORIGIN;
    whole := p.x * 10 + p.y;
    s := NAME;
    named := s = 'rk!';
    nested := TABLE[1].x;
    qualified := Shapes.CORNER.y;
    q := Shapes.CORNER;
    copied := q.x * 10 + q.y;
    through_using := Imported();
    inside := Shapes.Inside();
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";

/// Each read is the constant's value, part or whole.
#[rstest]
fn a_body_reads_an_aggregate_constant(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, AGGREGATES);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("a scan");
    let read = |path: &str| plc.read_var(&info, path);
    assert_eq!(read("Main.field"), Some(VarValue::I16(2)));
    assert_eq!(read("Main.element"), Some(VarValue::I16(3)));
    assert_eq!(read("Main.computed"), Some(VarValue::I16(5)));
    assert_eq!(read("Main.whole"), Some(VarValue::I16(12)));
    assert_eq!(read("Main.named"), Some(VarValue::Bool(true)));
    assert_eq!(read("Main.nested"), Some(VarValue::I16(7)));
    assert_eq!(read("Main.qualified"), Some(VarValue::I16(4)));
    assert_eq!(read("Main.copied"), Some(VarValue::I16(34)));
    assert_eq!(read("Main.through_using"), Some(VarValue::I16(3)));
    assert_eq!(read("Main.inside"), Some(VarValue::I16(4)));
}

/// A constant's value that names a constant called like a field of its
/// type: `x := y` is the constant `y`, not the field beside `x`.
const FIELD_NAMED: &str = "
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

VAR_CONSTANT
    y : INT := 5;
    P : Point := (x := y, y := 7);
END_VAR

PROGRAM Prog
VAR
    px : INT;
    py : INT;
END_VAR
    px := P.x;
    py := P.y;
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : Prog;
    END_RESOURCE
END_CONFIGURATION
";

#[rstest]
fn a_field_s_value_names_the_constant_not_the_field(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, FIELD_NAMED);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("a scan");
    assert_eq!(plc.read_var(&info, "Main.px"), Some(VarValue::I16(5)));
    assert_eq!(plc.read_var(&info, "Main.py"), Some(VarValue::I16(7)));
}

/// Partial accesses of constants: a bit, a byte, of a qualified constant,
/// through USING. Run without `__init`: a constant whose value is one
/// expression is that value, sliced where it is read, and holds no cell.
const SLICES: &str = "
VAR_CONSTANT
    FLAGS : WORD := 16#0008;
    BYTES : DWORD := 16#11223344;
END_VAR

NAMESPACE Ns
    VAR_CONSTANT
        MASK : BYTE := 16#04;
    END_VAR
END_NAMESPACE

FUNCTION ReadBit : BOOL
    ReadBit := FLAGS.%X3;
END_FUNCTION

FUNCTION ReadOther : BOOL
    ReadOther := FLAGS.%X2;
END_FUNCTION

FUNCTION ReadHigh : BYTE
    ReadHigh := BYTES.%B3;
END_FUNCTION

FUNCTION ReadQualified : BOOL
    ReadQualified := Ns.MASK.%X2;
END_FUNCTION

FUNCTION ReadUsing : BOOL
USING Ns;
    ReadUsing := MASK.%X1;
END_FUNCTION
";

#[rstest]
fn a_partial_access_is_a_slice_of_the_value(mut with_db: db::RootDatabase) {
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, SLICES);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "ReadBit", ()), 1);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "ReadOther", ()), 0);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "ReadHigh", ()), 0x11);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "ReadQualified", ()), 1);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "ReadUsing", ()), 0);
    assert_eq!(
        mir.constants_base,
        mir.memory_layout.total_size(),
        "no constant has a cell"
    );
}

/// A STRUCT input, a block's input and a STRING input given a constant:
/// each is the callee's copy, which it writes, and the constant keeps its
/// value, from one scan to the next.
const COPIES: &str = "
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

VAR_CONSTANT
    ORIGIN : Point := (x := 1, y := 2);
    NAME : STRING := 'rk!';
END_VAR

FUNCTION Clobber : INT
VAR_INPUT
    p : Point;
END_VAR
    p.x := 99;
    Clobber := p.x;
END_FUNCTION

FUNCTION ClobberText : INT
VAR_INPUT
    s : STRING;
END_VAR
    s := 'zz';
    ClobberText := 1;
END_FUNCTION

FUNCTION_BLOCK Holder
VAR_INPUT
    p : Point;
END_VAR
    p.y := 77;
END_FUNCTION_BLOCK

PROGRAM P
VAR
    h : Holder;
    seen : INT;
    after : INT;
    after_block : INT;
    text_after : STRING;
END_VAR
    seen := Clobber(p := ORIGIN);
    after := ORIGIN.x;
    h(p := ORIGIN);
    after_block := ORIGIN.y;
    ClobberText(s := NAME);
    text_after := NAME;
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";

#[rstest]
fn a_callee_writes_its_copy_of_a_constant(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, COPIES);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(2).expect("two scans");
    let read = |path: &str| plc.read_var(&info, path);
    assert_eq!(read("Main.seen"), Some(VarValue::I16(99)), "the copy");
    assert_eq!(read("Main.after"), Some(VarValue::I16(1)));
    assert_eq!(read("Main.after_block"), Some(VarValue::I16(2)));
    assert_eq!(
        read("Main.text_after"),
        Some(VarValue::String("rk!".to_string()))
    );
}

/// A test reads an aggregate constant: `rk test` runs `__init` before each
/// test, as a host runs it before the first scan. The second test must fail.
const TESTED: &str = "
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

VAR_CONSTANT
    ORIGIN : Point := (x := 1, y := 2);
END_VAR

{test}
FUNCTION ReadsOrigin : BOOL
    IF ORIGIN.y <> 2 THEN
        __RAISE('ORIGIN.y is not 2');
    END_IF
    ReadsOrigin := TRUE;
END_FUNCTION

{test}
FUNCTION ExpectsAnother : BOOL
    IF ORIGIN.y <> 3 THEN
        __RAISE('ORIGIN.y is not 3');
    END_IF
    ExpectsAnother := TRUE;
END_FUNCTION
";

#[rstest]
fn a_test_reads_an_aggregate_constant(mut with_db: db::RootDatabase) {
    let build = compile_tests(&mut with_db, TESTED);
    let records = run_tests(&build.component, None).expect("the tests run");
    let outcome = |name: &str| {
        let record = records.iter().find(|r| r.name == name).expect(name);
        (record.status, record.reason.clone())
    };
    assert_eq!(outcome("ReadsOrigin"), (Status::Pass, None));
    assert_eq!(
        outcome("ExpectsAnother"),
        (Status::Fail, Some("ORIGIN.y is not 3".to_string()))
    );
}

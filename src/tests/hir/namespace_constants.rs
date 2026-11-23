// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `VAR_CONSTANT`: the constants of a namespace, written at the top of a file
//! or in a NAMESPACE, which TYPEs and POUs name as they name its TYPEs.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, test_diagnostics_with_library, with_db};

/// A bound and a length name a constant of the file's top level: in a TYPE
/// and in a STRUCT element.
#[rstest]
fn valid_a_bound_names_a_constant_of_its_file(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
    LEN : INT := 32;
END_VAR

TYPE Row : ARRAY[0..ROWS - 1] OF INT; END_TYPE
TYPE Name : STRING[LEN]; END_TYPE
TYPE Pair : STRUCT
    cells : ARRAY[0..ROWS] OF INT;
    label : STRING[LEN];
END_STRUCT; END_TYPE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A bound names a constant of its namespace, or of a namespace around it.
#[rstest]
fn valid_a_bound_names_a_constant_of_its_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Limits
    VAR_CONSTANT
        LOW : DINT := 2;
        HIGH : DINT := 6;
    END_VAR
    TYPE Span : ARRAY[LOW..HIGH + 2] OF BYTE; END_TYPE
    NAMESPACE Inner
        TYPE Wide : ARRAY[0..HIGH * 2] OF BYTE; END_TYPE
    END_NAMESPACE
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A constant is a value: named where a type goes, it is refused (E0323),
/// as a FUNCTION is (E0316).
#[rstest]
fn invalid_a_constant_used_as_a_type(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
END_VAR

FUNCTION_BLOCK Grid
VAR
    cells : ROWS;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0323] Error: constant used as a type
       ,-[ file:///test0.st:8:13 ]
       |
     3 |     ROWS : INT := 10;
       |     ^^|^
       |       `--- 'ROWS' is declared here
       |
     8 |     cells : ROWS;
       |             ^^|^
       |               `--- 'ROWS' is a constant, not a type
    ---'
    ");
}

/// A constant shares its namespace's names with its POUs and TYPEs: named
/// like a FUNCTION or a PROGRAM of the root, like a TYPE of a NAMESPACE, or
/// like another constant there in another file, it is a duplicate (E0102),
/// as two POUs are.
#[rstest]
fn invalid_a_constant_named_like_an_item_of_its_namespace(mut with_db: RootDatabase) {
    let first = r#"
VAR_CONSTANT
    Helper : INT := 1;
    Main : INT := 2;
END_VAR

FUNCTION Helper : INT
    Helper := 0;
END_FUNCTION

PROGRAM Main
END_PROGRAM

NAMESPACE Limits
    TYPE Row : ARRAY[0..2] OF INT; END_TYPE
    VAR_CONSTANT
        Row : INT := 3;
        ROWS : INT := 4;
    END_VAR
END_NAMESPACE
"#;
    let second = r#"
NAMESPACE Limits
    VAR_CONSTANT
        rows : INT := 5;
    END_VAR
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[first, second]), @r"
    [E0102] Error: duplicate name
       ,-[ file:///test0.st:7:10 ]
       |
     3 |     Helper : INT := 1;
       |     ^^^|^^
       |        `---- constant 'Helper' is already declared here
       |
     7 | FUNCTION Helper : INT
       |          ^^^|^^
       |             `---- FUNCTION 'Helper' has the name of a constant
    ---'
    [E0102] Error: duplicate name
        ,-[ file:///test0.st:11:9 ]
        |
      4 |     Main : INT := 2;
        |     ^^|^
        |       `--- constant 'Main' is already declared here
        |
     11 | PROGRAM Main
        |         ^^|^
        |           `--- PROGRAM 'Main' has the name of a constant
    ----'
    [E0102] Error: duplicate name
        ,-[ file:///test0.st:17:9 ]
        |
     15 |     TYPE Row : ARRAY[0..2] OF INT; END_TYPE
        |          ^|^
        |           `--- TYPE 'Row' is already declared here
        |
     17 |         Row : INT := 3;
        |         ^|^
        |          `--- constant 'Row' has the name of a TYPE
    ----'
    [E0102] Error: duplicate name
        ,-[ file:///test1.st:4:9 ]
        |
      4 |         rows : INT := 5;
        |         ^^|^
        |           `--- constant 'rows' is declared twice
        |
        |-[ file:///test0.st:18:9 ]
        |
     18 |         ROWS : INT := 4;
        |         ^^|^
        |           `--- constant 'ROWS' is already declared here
    ----'
    ");
}

/// A bound is the value of the constant it names: an index past it says so,
/// `ROWS - 1` being 9, `LOW..HIGH + 2` being 2..8, and `HIGH * 2` in a
/// namespace inside being 12.
#[rstest]
fn invalid_an_index_past_a_bound_shows_what_the_constants_are(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
END_VAR

NAMESPACE Limits
    VAR_CONSTANT
        LOW : DINT := 2;
        HIGH : DINT := 6;
    END_VAR
    TYPE Span : ARRAY[LOW..HIGH + 2] OF BYTE; END_TYPE
    NAMESPACE Inner
        TYPE Wide : ARRAY[0..HIGH * 2] OF BYTE; END_TYPE
    END_NAMESPACE
END_NAMESPACE

TYPE Row : ARRAY[0..ROWS - 1] OF INT; END_TYPE

FUNCTION Past : INT
VAR
    r : Row;
    s : Limits.Span;
    w : Limits.Inner.Wide;
END_VAR
    r[10] := 1;
    s[9] := 1;
    w[13] := 1;
    Past := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:25:7 ]
        |
     25 |     r[10] := 1;
        |       ^|
        |        `-- index 10 is out of bounds (the dimension is declared 0..9)
    ----'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:26:7 ]
        |
     26 |     s[9] := 1;
        |       |
        |       `-- index 9 is out of bounds (the dimension is declared 2..8)
    ----'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:27:7 ]
        |
     27 |     w[13] := 1;
        |       ^|
        |        `-- index 13 is out of bounds (the dimension is declared 0..12)
    ----'
    ");
}

/// A constant whose value reads itself, directly or through another, is
/// refused where it is declared (E0408), and a bound naming it says
/// nothing more.
#[rstest]
fn invalid_a_constant_defined_by_its_own_value(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    A : INT := A + 1;
END_VAR

NAMESPACE Limits
    VAR_CONSTANT
        B : INT := C * 2;
        C : INT := B;
    END_VAR
    TYPE Col : ARRAY[0..B] OF INT; END_TYPE
END_NAMESPACE

NAMESPACE Ring
    VAR_CONSTANT
        X : INT := Y;
        Y : INT := Z + 1;
        Z : INT := X;
    END_VAR
END_NAMESPACE

TYPE Row : ARRAY[0..A] OF INT; END_TYPE
TYPE Name : STRING[A]; END_TYPE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0408] Error: constant defined by its own value
       ,-[ file:///test0.st:3:16 ]
       |
     3 |     A : INT := A + 1;
       |                |
       |                `-- 'A' is defined by its own value
       |
       | Note: a constant's value is computed before the program runs, from the constants it names
    ---'
    [E0408] Error: constant defined by its own value
       ,-[ file:///test0.st:8:20 ]
       |
     8 |         B : INT := C * 2;
       |                    |
       |                    `-- 'B' is defined by its own value, through 'C'
     9 |         C : INT := B;
       |         |
       |         `-- 'C' is declared here
       |
       | Note: a constant's value is computed before the program runs, from the constants it names
    ---'
    [E0408] Error: constant defined by its own value
       ,-[ file:///test0.st:9:20 ]
       |
     8 |         B : INT := C * 2;
       |         |
       |         `-- 'B' is declared here
     9 |         C : INT := B;
       |                    |
       |                    `-- 'C' is defined by its own value, through 'B'
       |
       | Note: a constant's value is computed before the program runs, from the constants it names
    ---'
    [E0408] Error: constant defined by its own value
        ,-[ file:///test0.st:16:20 ]
        |
     16 |         X : INT := Y;
        |                    |
        |                    `-- 'X' is defined by its own value, through 'Y' and 'Z'
     17 |         Y : INT := Z + 1;
        |         |
        |         `-- 'Y' is declared here
     18 |         Z : INT := X;
        |         |
        |         `-- 'Z' is declared here
        |
        | Note: a constant's value is computed before the program runs, from the constants it names
    ----'
    [E0408] Error: constant defined by its own value
        ,-[ file:///test0.st:17:20 ]
        |
     16 |         X : INT := Y;
        |         |
        |         `-- 'X' is declared here
     17 |         Y : INT := Z + 1;
        |                    |
        |                    `-- 'Y' is defined by its own value, through 'Z' and 'X'
     18 |         Z : INT := X;
        |         |
        |         `-- 'Z' is declared here
        |
        | Note: a constant's value is computed before the program runs, from the constants it names
    ----'
    [E0408] Error: constant defined by its own value
        ,-[ file:///test0.st:18:20 ]
        |
     16 |         X : INT := Y;
        |         |
        |         `-- 'X' is declared here
     17 |         Y : INT := Z + 1;
        |         |
        |         `-- 'Y' is declared here
     18 |         Z : INT := X;
        |                    |
        |                    `-- 'Z' is defined by its own value, through 'X' and 'Y'
        |
        | Note: a constant's value is computed before the program runs, from the constants it names
    ----'
    ");
}

/// A constant's value is computed before the program runs: one that calls
/// a FUNCTION is refused, as a TYPE's default would be.
#[rstest]
fn invalid_a_constant_whose_value_is_no_constant(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Helper : INT
    Helper := 1;
END_FUNCTION

VAR_CONSTANT
    N : INT := Helper();
END_VAR
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:7:16 ]
       |
     7 |     N : INT := Helper();
       |                ^^^^|^^^
       |                    `----- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: a call is not a constant
    ---'
    ");
}

/// A POU names a constant as it names a TYPE: a bound and a body read one of
/// the root namespace from another file, of its own namespace bare, of
/// another as `Limits.HIGH`, or through `USING Limits`.
#[rstest]
fn valid_a_pou_names_a_constant_as_it_names_a_type(mut with_db: RootDatabase) {
    let constants = r#"
VAR_CONSTANT
    ROWS : INT := 10;
END_VAR

NAMESPACE Limits
    VAR_CONSTANT
        HIGH : INT := 6;
    END_VAR
    FUNCTION Inside : INT
    VAR cells : ARRAY[0..HIGH] OF INT; END_VAR
        cells[HIGH] := HIGH;
        Inside := cells[HIGH];
    END_FUNCTION
END_NAMESPACE
"#;
    let uses = r#"
TYPE Row : ARRAY[0..ROWS] OF INT; END_TYPE

FUNCTION Bare : INT
VAR cells : ARRAY[1..ROWS] OF INT; END_VAR
    cells[ROWS] := ROWS;
    Bare := cells[ROWS];
END_FUNCTION

FUNCTION Qualified : INT
VAR cells : ARRAY[0..Limits.HIGH] OF INT; END_VAR
    Qualified := Limits.HIGH;
END_FUNCTION

FUNCTION Imported : INT
USING Limits;
VAR cells : ARRAY[0..HIGH] OF INT; END_VAR
    Imported := HIGH;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[constants, uses]), @"");
}

/// A library's constant is named as its TYPEs are: bare, every library
/// being imported, or from its root.
#[rstest]
fn valid_a_library_s_constant(mut with_db: RootDatabase) {
    let library = r#"
VAR_CONSTANT
    LIMIT : INT := 8;
END_VAR
"#;
    let source = r#"
FUNCTION Uses : INT
VAR cells : ARRAY[0..Std.LIMIT] OF INT; END_VAR
    Uses := LIMIT + Std.LIMIT;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics_with_library(&mut with_db, &[library], &[source]), @"");
}

/// A constant is read, never written: an assignment, a VAR_IN_OUT and a
/// reference are refused, as for a POU's own constant (E0404).
#[rstest]
fn invalid_a_constant_is_not_written(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
END_VAR

FUNCTION Bump : INT
VAR_IN_OUT n : INT; END_VAR
    n := n + 1;
    Bump := n;
END_FUNCTION

FUNCTION Writes : INT
VAR r : REF_TO INT; END_VAR
    ROWS := 1;
    Writes := Bump(n := ROWS);
    r := REF(ROWS);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:14:5 ]
        |
      3 |     ROWS : INT := 10;
        |     ^^|^
        |       `--- 'ROWS' is declared CONSTANT here
        |
     14 |     ROWS := 1;
        |     ^^|^
        |       `--- cannot write to constant 'ROWS'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:15:25 ]
        |
      3 |     ROWS : INT := 10;
        |     ^^|^
        |       `--- 'ROWS' is declared CONSTANT here
        |
     15 |     Writes := Bump(n := ROWS);
        |                         ^^|^
        |                           `--- cannot pass constant 'ROWS' to a VAR_IN_OUT
        |
        | Help: pass it to a VAR_INPUT, or copy it into a variable and pass that
        |
        | Note: a VAR_IN_OUT could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:16:14 ]
        |
      3 |     ROWS : INT := 10;
        |     ^^|^
        |       `--- 'ROWS' is declared CONSTANT here
        |
     16 |     r := REF(ROWS);
        |              ^^|^
        |                `--- cannot take a reference to constant 'ROWS'
        |
        | Help: copy it into a variable and take the reference of that
        |
        | Note: a reference could change it
    ----'
    ");
}

/// Two USINGs that each bring a constant of one name leave it ambiguous
/// (E0205), as two TYPEs would.
#[rstest]
fn invalid_two_usings_bring_a_constant_of_one_name(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE A
    VAR_CONSTANT N : INT := 1; END_VAR
END_NAMESPACE

NAMESPACE B
    VAR_CONSTANT N : INT := 2; END_VAR
END_NAMESPACE

FUNCTION Both : INT
USING A, B;
    Both := N;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0205] Error: ambiguous name
        ,-[ file:///test0.st:12:13 ]
        |
     12 |     Both := N;
        |             |
        |             `-- multiple items named 'N' available in scope
        |
        | Help: qualify the name: A.N or B.N
    ----'
    ");
}

/// A constant of the program's root named as a library's hides it, as a
/// TYPE and a FUNCTION do: the program's `N`, `Shape` and `Twice` are read
/// (the library's would be a STRING, a `Shape` with no `b`), and a library
/// that adds a name breaks no program. Two libraries answering is E0205.
#[rstest]
fn valid_a_program_s_constant_hides_a_library_s(mut with_db: RootDatabase) {
    let library = r#"
VAR_CONSTANT
    N : STRING := 'library';
END_VAR
TYPE Shape : STRUCT a : INT; END_STRUCT; END_TYPE
FUNCTION Twice : STRING
    Twice := 'library';
END_FUNCTION
"#;
    let source = r#"
VAR_CONSTANT
    N : INT := 2;
END_VAR
TYPE Shape : STRUCT b : INT; END_STRUCT; END_TYPE
FUNCTION Twice : INT
    Twice := 2;
END_FUNCTION
FUNCTION Reads : INT
VAR
    s : Shape;
END_VAR
    s.b := N;
    Reads := s.b + Twice();
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics_with_library(&mut with_db, &[library], &[source]), @r"");
}

/// A partial access of a constant is a slice of its value, held to the
/// constant's width as a variable's is.
#[rstest]
fn invalid_a_partial_access_past_a_constant_s_width(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    FLAGS : WORD := 16#0008;
END_VAR

FUNCTION Bits : BOOL
    Bits := FLAGS.%X3 AND FLAGS.%X16;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1429] Error: partial access out of range
       ,-[ file:///test0.st:7:27 ]
       |
     3 |     FLAGS : WORD := 16#0008;
       |     ^^|^^
       |       `---- 'FLAGS' is declared here
       |
     7 |     Bits := FLAGS.%X3 AND FLAGS.%X16;
       |                           ^^|^^
       |                             `---- offset 16 is out of range for type 'WORD' (valid range: 0..15)
    ---'
    ");
}

/// An initial value that names a part of a constant, a slice or a field, or
/// a whole STRUCT, is refused (E0401): an initial value folds, and these do
/// not. A FUNCTION's local is set at each call and may.
#[rstest]
fn invalid_an_initial_value_of_a_constant_s_part(mut with_db: RootDatabase) {
    let source = r#"
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

VAR_CONSTANT
    FLAGS : WORD := 16#0008;
    ORIGIN : Point := (x := 1, y := 2);
    BIT : BOOL := FLAGS.%X3;
END_VAR

PROGRAM P
VAR
    copy : Point := ORIGIN;
    y : INT := ORIGIN.y;
END_VAR
END_PROGRAM

FUNCTION Local : INT
VAR
    l : Point := ORIGIN;
END_VAR
    Local := l.y;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:7:19 ]
       |
     7 |     BIT : BOOL := FLAGS.%X3;
       |                   ^^^^|^^^^
       |                       `------ this initial value must be a constant: it is fixed before the program runs
    ---'
    [E0401] Error: initial value not constant
        ,-[ file:///test0.st:12:21 ]
        |
     12 |     copy : Point := ORIGIN;
        |                     ^^^|^^
        |                        `---- this initial value must be a constant: it is fixed before the program runs
        |
        | Note: 'ORIGIN' is CONSTANT, but its own value does not fold (a reference cycle, or a non-constant initializer)
    ----'
    [E0401] Error: initial value not constant
        ,-[ file:///test0.st:13:16 ]
        |
     13 |     y : INT := ORIGIN.y;
        |                ^^^^|^^^
        |                    `----- this initial value must be a constant: it is fixed before the program runs
    ----'
    ");
}

/// A task's INTERVAL names a constant as a body does, bare, qualified or
/// through USING, and the schedule holds its value.
#[rstest]
fn valid_a_task_s_interval_names_a_constant(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    FAST : TIME := T#5ms;
END_VAR

NAMESPACE Cycle
    VAR_CONSTANT
        SLOW : TIME := T#100ms;
        SLOWER : LTIME := LTIME#1s;
    END_VAR
END_NAMESPACE

USING Cycle;

PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK Quick(INTERVAL := FAST, PRIORITY := 1);
        TASK Steady(INTERVAL := Cycle.SLOW, PRIORITY := 2);
        TASK Lazy(INTERVAL := SLOWER, PRIORITY := 3);
        PROGRAM P1 WITH Quick : P;
        PROGRAM P2 WITH Steady : P;
        PROGRAM P3 WITH Lazy : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
    assert_snapshot!(crate::tests::hir::config::resolved_schedule(&with_db), @r"
    RESOURCE R ON CPU
      TASK Quick every 5ms, priority 1
        PROGRAM P1 : P
      TASK Steady every 100ms, priority 2
        PROGRAM P2 : P
      TASK Lazy every 1s, priority 3
        PROGRAM P3 : P
    ");
}

/// A constant that holds no TIME gives no period, and a constant SINGLE never
/// rises, as for a CONSTANT global (E1410).
#[rstest]
fn invalid_a_task_parameter_s_constant(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    COUNT : INT := 5;
    ALWAYS : BOOL := TRUE;
END_VAR

PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK Counted(INTERVAL := COUNT, PRIORITY := 1);
        TASK Once(SINGLE := ALWAYS, PRIORITY := 2);
        PROGRAM P1 WITH Counted : P;
        PROGRAM P2 WITH Once : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1410] Error: unschedulable TASK
        ,-[ file:///test0.st:11:14 ]
        |
     11 |         TASK Counted(INTERVAL := COUNT, PRIORITY := 1);
        |              ^^^|^^^
        |                 `----- task 'Counted' cannot be scheduled: INTERVAL names a constant that holds no TIME literal
        |
        | Help: give the constant a TIME literal, such as `T#10ms`
    ----'
    [E1410] Error: unschedulable TASK
        ,-[ file:///test0.st:12:29 ]
        |
     12 |         TASK Once(SINGLE := ALWAYS, PRIORITY := 2);
        |                             ^^^|^^
        |                                `---- task 'Once' cannot be scheduled: SINGLE is always TRUE, so the task would run once
        |
        | Help: name a BOOL variable or a bit address
        |
        | Note: a SINGLE already TRUE at its first reading is a rising edge, and an INTERVAL runs while SINGLE stands at FALSE
    ----'
    ");
}

// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The statements after a RETURN, EXIT, CONTINUE or `__RAISE` in the same
//! block are dead, and L0101 says so. They are checked all the same: the
//! walk used to stop at the terminator, so an undeclared name or a mismatch
//! after one passed `rk check`, and lowering, which walks every statement,
//! found no type for them and failed as a compiler bug.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// Every error a live statement would report, reported in a dead one: an
/// unknown name, a literal of the wrong kind, an unknown field and an
/// unknown method, after each of the four terminators.
#[rstest]
fn invalid_dead_code_is_checked(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Fb
METHOD PUBLIC m : INT
    m := 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION f : INT
VAR v : INT; p : Pt; fb : Fb; i : INT; END_VAR
    FOR i := 0 TO 2 DO
        EXIT;
        v := fb.unknown();
    END_FOR;
    FOR i := 0 TO 2 DO
        CONTINUE;
        v := p.y;
    END_FOR;
    IF v = 1 THEN
        __RAISE('stop');
        v := 'text';
    END_IF;
    RETURN;
    v := undeclared + 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0202] Error: unknown field
        ,-[ file:///test0.st:14:17 ]
        |
      4 | FUNCTION_BLOCK Fb
        |                ^|
        |                 `-- FUNCTION_BLOCK 'Fb' is declared here
        |
     14 |         v := fb.unknown();
        |                 ^^^|^^^
        |                    `----- 'Fb' has no field named 'unknown'
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:18:16 ]
        |
      2 | TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE
        |      ^|
        |       `-- 'Pt' is declared here
        |
     18 |         v := p.y;
        |                |
        |                `-- 'Pt' has no field named 'y'
        |
        | Note: 'Pt' has a field with a similar name:
        |       - x
    ----'
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:22:14 ]
        |
     11 | VAR v : INT; p : Pt; fb : Fb; i : INT; END_VAR
        |     |
        |     `-- 'v' is declared here
        |
     22 |         v := 'text';
        |              ^^^|^^
        |                 `---- cannot use string literal as INT
    ----'
    [E0201] Error: unknown name
        ,-[ file:///test0.st:25:10 ]
        |
     25 |     v := undeclared + 1;
        |          ^^^^^|^^^^
        |               `------ no item 'undeclared' found in scope
    ----'
    ");
}

/// Dead code the checker accepts compiles: each shape here was an internal
/// compiler error at lowering, which walked the statements the checker had
/// skipped. `test_diagnostics` lowers and emits an accepted program.
#[rstest]
fn valid_dead_code_compiles(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT f0 : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Fb
METHOD PUBLIC m0 : INT
    m0 := 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION f1 : INT
VAR v : DINT; END_VAR
    f1 := 0;
    RETURN;
    v := v + 1;
END_FUNCTION

FUNCTION f2 : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 3 DO
        EXIT;
        FOR i := 0 TO 1 DO
            f2 := i;
        END_FOR;
    END_FOR;
END_FUNCTION

FUNCTION f3 : INT
VAR s : Pt; END_VAR
    f3 := 0;
    RETURN;
    s.f0 := 1;
    f3 := s.f0;
END_FUNCTION

FUNCTION f4 : INT
VAR v : INT; END_VAR
VAR_IN_OUT fb : Fb; END_VAR
    f4 := 0;
    RETURN;
    v := fb.m0();
    fb.m0();
END_FUNCTION

FUNCTION f5 : INT
VAR v : INT; END_VAR
    __RAISE('x');
    v := v + 1;
    f5 := v;
END_FUNCTION

FUNCTION f6 : INT
VAR i : INT; v : INT; END_VAR
    FOR i := 0 TO 3 DO
        CONTINUE;
        v := v + 1;
    END_FOR;
    IF v = 1 THEN
        RETURN;
        v := 2;
    END_IF;
    f6 := v;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
